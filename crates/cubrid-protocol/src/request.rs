//! Request packet builders for CAS function codes.

use crate::codec::PacketWriter;
use crate::constants::*;
use crate::value::Value;

/// Build a PrepareAndExecute request (FC=41).
///
/// This is a combined prepare + execute in one round trip (no bind parameters).
pub fn write_prepare_and_execute(
    sql: &str,
    auto_commit: bool,
    cas_info: &[u8; SIZE_CAS_INFO],
) -> Vec<u8> {
    let mut w = PacketWriter::with_capacity(256);
    w.write_byte(FunctionCode::PrepareAndExecute as u8);
    w.add_int(3); // arg count
    w.write_null_term_string(sql);
    w.add_byte(PREPARE_NORMAL);
    w.add_byte(if auto_commit { 1 } else { 0 });
    w.add_byte(EXECUTE_QUERY_ALL);
    w.add_int(0); // max_col_size
    w.add_int(0); // max_row_size
    w.write_int(0); // NULL bind params (raw — no length prefix)
    w.write_int(SIZE_LONG as i32); // cache time length
    w.write_int(0); // cache_time_sec
    w.write_int(0); // cache_time_usec
    w.add_int(0); // query timeout
    PacketWriter::build_request(w.as_bytes(), cas_info)
}

/// Build a Prepare request (FC=2).
pub fn write_prepare(sql: &str, auto_commit: bool, cas_info: &[u8; SIZE_CAS_INFO]) -> Vec<u8> {
    let mut w = PacketWriter::with_capacity(128);
    w.write_byte(FunctionCode::Prepare as u8);
    w.write_null_term_string(sql);
    w.add_byte(PREPARE_NORMAL);
    w.add_byte(if auto_commit { 1 } else { 0 });
    PacketWriter::build_request(w.as_bytes(), cas_info)
}

/// Build an Execute request (FC=3).
pub fn write_execute(
    query_handle: i32,
    stmt_type: StatementType,
    args: &[Value],
    auto_commit: bool,
    cas_info: &[u8; SIZE_CAS_INFO],
) -> Vec<u8> {
    let fetch_flag: u8 = if stmt_type.is_select() { 1 } else { 0 };

    let mut w = PacketWriter::with_capacity(256);
    w.write_byte(FunctionCode::Execute as u8);
    w.add_int(query_handle);
    w.add_byte(EXECUTE_NORMAL);
    w.add_int(0); // max_col_size
    w.add_int(0); // max_row_size

    // Bind parameters
    let bind_data = encode_bind_params(args);
    if bind_data.is_empty() {
        w.write_int(0); // raw int32(0) = no bind params
    } else {
        w.add_bytes(&bind_data);
    }

    w.write_int(1); // bind_mode_count (raw)
    w.write_byte(fetch_flag); // fetch flag (raw)
    w.add_byte(if auto_commit { 1 } else { 0 });
    w.add_byte(1); // forward_only
    w.add_cache_time();
    w.add_int(0); // query timeout

    PacketWriter::build_request(w.as_bytes(), cas_info)
}

/// Build a Fetch request (FC=8).
pub fn write_fetch(
    query_handle: i32,
    current_tuple_count: i32,
    fetch_size: i32,
    cas_info: &[u8; SIZE_CAS_INFO],
) -> Vec<u8> {
    let mut w = PacketWriter::with_capacity(64);
    w.write_byte(FunctionCode::Fetch as u8);
    w.add_int(query_handle);
    w.add_int(current_tuple_count + 1); // 1-based start position
    w.add_int(fetch_size);
    w.add_byte(0); // case_sensitive
    w.add_int(0); // resultset_index
    PacketWriter::build_request(w.as_bytes(), cas_info)
}

/// Build a CloseReqHandle request (FC=6).
pub fn write_close_req_handle(query_handle: i32, cas_info: &[u8; SIZE_CAS_INFO]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_byte(FunctionCode::CloseReqHandle as u8);
    w.add_int(query_handle);
    PacketWriter::build_request(w.as_bytes(), cas_info)
}

/// Build a GetLastInsertId request (FC=40).
pub fn write_get_last_insert_id(cas_info: &[u8; SIZE_CAS_INFO]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_byte(FunctionCode::GetLastInsertId as u8);
    PacketWriter::build_request(w.as_bytes(), cas_info)
}

/// Encode bind parameters into wire format.
///
/// Each parameter is encoded as:
/// - NULL: `[i32(0)]`
/// - Non-null: `[i32(1 + value_size)] [u8 type_code] [value_bytes]`
pub fn encode_bind_params(params: &[Value]) -> Vec<u8> {
    if params.is_empty() {
        return Vec::new();
    }
    let mut w = PacketWriter::with_capacity(params.len() * 16);
    for param in params {
        encode_one_param(&mut w, param);
    }
    w.into_bytes()
}

fn encode_one_param(w: &mut PacketWriter, value: &Value) {
    match value {
        Value::Null => {
            w.write_int(0); // raw int32(0) = NULL
        }
        Value::Bool(v) => {
            w.write_int((1 + SIZE_SHORT) as i32); // 3
            w.write_byte(DataType::Short as u8);
            w.write_short(if *v { 1 } else { 0 });
        }
        Value::Short(v) => {
            w.write_int((1 + SIZE_SHORT) as i32);
            w.write_byte(DataType::Short as u8);
            w.write_short(*v);
        }
        Value::Int(v) => {
            w.write_int((1 + SIZE_INT) as i32); // 5
            w.write_byte(DataType::Int as u8);
            w.write_int(*v);
        }
        Value::Long(v) => {
            w.write_int((1 + SIZE_LONG) as i32); // 9
            w.write_byte(DataType::Bigint as u8);
            w.write_long(*v);
        }
        Value::Float(v) => {
            w.write_int((1 + SIZE_FLOAT) as i32); // 5
            w.write_byte(DataType::Float as u8);
            w.write_float(*v);
        }
        Value::Double(v) => {
            w.write_int((1 + SIZE_DOUBLE) as i32); // 9
            w.write_byte(DataType::Double as u8);
            w.write_double(*v);
        }
        Value::String(v) => {
            let bytes = v.as_bytes();
            w.write_int((1 + bytes.len() + 1) as i32); // type + string + null
            w.write_byte(DataType::String as u8);
            w.write_raw_bytes(bytes);
            w.write_byte(0x00); // null terminator
        }
        Value::Bytes(v) => {
            w.write_int((1 + v.len()) as i32);
            w.write_byte(DataType::VarBit as u8);
            w.write_raw_bytes(v);
        }
        Value::Date { year, month, day } => {
            w.write_int((1 + SIZE_DATETIME) as i32); // 15
            w.write_byte(DataType::Datetime as u8);
            w.write_short(*year);
            w.write_short(*month);
            w.write_short(*day);
            w.write_short(0); // hour
            w.write_short(0); // minute
            w.write_short(0); // second
            w.write_short(0); // ms
        }
        Value::Datetime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            ms,
        } => {
            w.write_int((1 + SIZE_DATETIME) as i32); // 15
            w.write_byte(DataType::Datetime as u8);
            w.write_short(*year);
            w.write_short(*month);
            w.write_short(*day);
            w.write_short(*hour);
            w.write_short(*minute);
            w.write_short(*second);
            w.write_short(*ms);
        }
        Value::Time {
            hour,
            minute,
            second,
        } => {
            w.write_int((1 + SIZE_DATETIME) as i32); // 15
            w.write_byte(DataType::Time as u8);
            w.write_short(0); // year
            w.write_short(0); // month
            w.write_short(0); // day
            w.write_short(*hour);
            w.write_short(*minute);
            w.write_short(*second);
            w.write_short(0); // ms
        }
        Value::Timestamp {
            year,
            month,
            day,
            hour,
            minute,
            second,
        } => {
            w.write_int((1 + SIZE_DATETIME) as i32); // 15
            w.write_byte(DataType::Timestamp as u8);
            w.write_short(*year);
            w.write_short(*month);
            w.write_short(*day);
            w.write_short(*hour);
            w.write_short(*minute);
            w.write_short(*second);
            w.write_short(0); // ms
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prepare_and_execute_starts_with_header() {
        let cas_info = [0x01, 0x02, 0x03, 0x04];
        let buf = write_prepare_and_execute("SELECT 1", true, &cas_info);
        // First 4 bytes: DATA_LENGTH
        let data_len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        // Next 4 bytes: CAS_INFO
        assert_eq!(&buf[4..8], &cas_info);
        // Payload starts at offset 8
        assert_eq!(buf.len(), 8 + data_len);
        // First byte of payload: function code
        assert_eq!(buf[8], FunctionCode::PrepareAndExecute as u8);
    }

    #[test]
    fn test_prepare_starts_with_header() {
        let cas_info = [0; SIZE_CAS_INFO];
        let buf = write_prepare("SELECT 1", false, &cas_info);
        assert_eq!(buf[8], FunctionCode::Prepare as u8);
    }

    #[test]
    fn test_execute_starts_with_header() {
        let cas_info = [0; SIZE_CAS_INFO];
        let buf = write_execute(1, StatementType::Select, &[], true, &cas_info);
        assert_eq!(buf[8], FunctionCode::Execute as u8);
    }

    #[test]
    fn test_fetch_starts_with_header() {
        let cas_info = [0; SIZE_CAS_INFO];
        let buf = write_fetch(1, 0, 100, &cas_info);
        assert_eq!(buf[8], FunctionCode::Fetch as u8);
    }

    #[test]
    fn test_close_req_handle() {
        let cas_info = [0; SIZE_CAS_INFO];
        let buf = write_close_req_handle(42, &cas_info);
        assert_eq!(buf[8], FunctionCode::CloseReqHandle as u8);
    }

    #[test]
    fn test_get_last_insert_id() {
        let cas_info = [0; SIZE_CAS_INFO];
        let buf = write_get_last_insert_id(&cas_info);
        assert_eq!(buf[8], FunctionCode::GetLastInsertId as u8);
    }

    #[test]
    fn test_encode_bind_params_empty() {
        assert!(encode_bind_params(&[]).is_empty());
    }

    #[test]
    fn test_encode_bind_null() {
        let data = encode_bind_params(&[Value::Null]);
        assert_eq!(data, 0i32.to_be_bytes());
    }

    #[test]
    fn test_encode_bind_bool() {
        let data = encode_bind_params(&[Value::Bool(true)]);
        let mut expected = Vec::new();
        expected.extend_from_slice(&3i32.to_be_bytes()); // 1 + 2
        expected.push(DataType::Short as u8);
        expected.extend_from_slice(&1i16.to_be_bytes());
        assert_eq!(data, expected);
    }

    #[test]
    fn test_encode_bind_long() {
        let data = encode_bind_params(&[Value::Long(42)]);
        let mut expected = Vec::new();
        expected.extend_from_slice(&9i32.to_be_bytes()); // 1 + 8
        expected.push(DataType::Bigint as u8);
        expected.extend_from_slice(&42i64.to_be_bytes());
        assert_eq!(data, expected);
    }

    #[test]
    fn test_encode_bind_string() {
        let data = encode_bind_params(&[Value::String("hi".to_string())]);
        let mut expected = Vec::new();
        expected.extend_from_slice(&4i32.to_be_bytes()); // 1 + 2 + 1
        expected.push(DataType::String as u8);
        expected.extend_from_slice(b"hi");
        expected.push(0x00);
        assert_eq!(data, expected);
    }

    #[test]
    fn test_encode_bind_bytes() {
        let data = encode_bind_params(&[Value::Bytes(vec![0xAA, 0xBB])]);
        let mut expected = Vec::new();
        expected.extend_from_slice(&3i32.to_be_bytes()); // 1 + 2
        expected.push(DataType::VarBit as u8);
        expected.extend_from_slice(&[0xAA, 0xBB]);
        assert_eq!(data, expected);
    }

    #[test]
    fn test_encode_bind_double() {
        let data = encode_bind_params(&[Value::Double(std::f64::consts::PI)]);
        let mut expected = Vec::new();
        expected.extend_from_slice(&9i32.to_be_bytes());
        expected.push(DataType::Double as u8);
        expected.extend_from_slice(&std::f64::consts::PI.to_be_bytes());
        assert_eq!(data, expected);
    }

    #[test]
    fn test_encode_bind_datetime() {
        let data = encode_bind_params(&[Value::Datetime {
            year: 2024,
            month: 3,
            day: 15,
            hour: 10,
            minute: 30,
            second: 45,
            ms: 123,
        }]);
        let mut expected = Vec::new();
        expected.extend_from_slice(&15i32.to_be_bytes()); // 1 + 14
        expected.push(DataType::Datetime as u8);
        for v in [2024i16, 3, 15, 10, 30, 45, 123] {
            expected.extend_from_slice(&v.to_be_bytes());
        }
        assert_eq!(data, expected);
    }

    #[test]
    fn test_encode_multiple_params() {
        let params = vec![
            Value::Int(42),
            Value::String("hello".to_string()),
            Value::Null,
        ];
        let data = encode_bind_params(&params);
        // Should contain all three params sequentially
        assert!(!data.is_empty());

        // Verify first param: Int(42) = i32(5) + DataType::Int + i32(42)
        let size = i32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        assert_eq!(size, 5); // 1 + 4
    }

    #[test]
    fn test_encode_bind_int() {
        let data = encode_bind_params(&[Value::Int(42)]);
        let mut expected = Vec::new();
        expected.extend_from_slice(&5i32.to_be_bytes()); // 1 + 4
        expected.push(DataType::Int as u8);
        expected.extend_from_slice(&42i32.to_be_bytes());
        assert_eq!(data, expected);
    }

    #[test]
    fn test_encode_bind_short() {
        let data = encode_bind_params(&[Value::Short(100)]);
        let mut expected = Vec::new();
        expected.extend_from_slice(&3i32.to_be_bytes()); // 1 + 2
        expected.push(DataType::Short as u8);
        expected.extend_from_slice(&100i16.to_be_bytes());
        assert_eq!(data, expected);
    }

    #[test]
    fn test_encode_bind_float() {
        let data = encode_bind_params(&[Value::Float(1.5)]);
        let mut expected = Vec::new();
        expected.extend_from_slice(&5i32.to_be_bytes()); // 1 + 4
        expected.push(DataType::Float as u8);
        expected.extend_from_slice(&1.5f32.to_be_bytes());
        assert_eq!(data, expected);
    }

    #[test]
    fn test_encode_bind_date() {
        let data = encode_bind_params(&[Value::Date {
            year: 2024,
            month: 6,
            day: 15,
        }]);
        let mut expected = Vec::new();
        expected.extend_from_slice(&15i32.to_be_bytes()); // 1 + 14
        expected.push(DataType::Datetime as u8);
        for v in [2024i16, 6, 15, 0, 0, 0, 0] {
            expected.extend_from_slice(&v.to_be_bytes());
        }
        assert_eq!(data, expected);
    }

    #[test]
    fn test_encode_bind_time() {
        let data = encode_bind_params(&[Value::Time {
            hour: 14,
            minute: 30,
            second: 45,
        }]);
        let mut expected = Vec::new();
        expected.extend_from_slice(&15i32.to_be_bytes()); // 1 + 14
        expected.push(DataType::Time as u8);
        for v in [0i16, 0, 0, 14, 30, 45, 0] {
            expected.extend_from_slice(&v.to_be_bytes());
        }
        assert_eq!(data, expected);
    }

    #[test]
    fn test_encode_bind_timestamp() {
        let data = encode_bind_params(&[Value::Timestamp {
            year: 2024,
            month: 6,
            day: 15,
            hour: 14,
            minute: 30,
            second: 45,
        }]);
        let mut expected = Vec::new();
        expected.extend_from_slice(&15i32.to_be_bytes()); // 1 + 14
        expected.push(DataType::Timestamp as u8);
        for v in [2024i16, 6, 15, 14, 30, 45, 0] {
            expected.extend_from_slice(&v.to_be_bytes());
        }
        assert_eq!(data, expected);
    }
}
