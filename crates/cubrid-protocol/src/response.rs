//! Response parsers for CAS function codes.
//!
//! Each parser takes a raw response payload (starting with `CAS_INFO`) and
//! returns a typed result struct or a [`ProtocolError`].

use crate::codec::PacketReader;
use crate::constants::*;
use crate::error::ProtocolError;
use crate::types::*;
use crate::value::Value;

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Parse column metadata entries from the reader.
///
/// Wire format per column:
/// ```text
/// [u8 legacy_type]           — if bit 0x80 set, read next byte as actual type
/// [i16 scale]
/// [i32 precision]
/// [i32 name_len] [name_bytes]
/// [i32 real_name_len] [real_name_bytes]
/// [i32 table_name_len] [table_name_bytes]
/// [u8 is_nullable]
/// [i32 default_len] [default_bytes]
/// [u8 is_auto_increment]
/// [u8 is_unique_key]
/// [u8 is_primary_key]
/// [u8 is_reverse_index]      — skipped
/// [u8 is_reverse_unique]     — skipped
/// [u8 is_foreign_key]
/// [u8 is_shared]             — skipped
/// ```
pub fn parse_column_metadata(
    reader: &mut PacketReader<'_>,
    count: i32,
) -> Result<Vec<ColumnMetaData>, ProtocolError> {
    let mut cols = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let legacy_type = reader.parse_byte()?;
        let col_type = if legacy_type & 0x80 != 0 {
            reader.parse_byte()?
        } else {
            legacy_type
        };
        let data_type =
            DataType::from_u8(col_type).ok_or(ProtocolError::UnsupportedType(col_type))?;

        let scale = reader.parse_short()?;
        let precision = reader.parse_int()?;

        let name_len = reader.parse_int()? as usize;
        let name = reader.parse_null_term_string(name_len)?;
        let real_name_len = reader.parse_int()? as usize;
        let real_name = reader.parse_null_term_string(real_name_len)?;
        let table_name_len = reader.parse_int()? as usize;
        let table_name = reader.parse_null_term_string(table_name_len)?;

        let is_nullable = reader.parse_byte()? == 1;
        let default_len = reader.parse_int()? as usize;
        let default_value = reader.parse_null_term_string(default_len)?;
        let is_auto_increment = reader.parse_byte()? == 1;
        let is_unique_key = reader.parse_byte()? == 1;
        let is_primary_key = reader.parse_byte()? == 1;
        reader.parse_byte()?; // is_reverse_index — skip
        reader.parse_byte()?; // is_reverse_unique — skip
        let is_foreign_key = reader.parse_byte()? == 1;
        reader.parse_byte()?; // is_shared — skip

        cols.push(ColumnMetaData {
            column_type: data_type,
            scale,
            precision,
            name,
            real_name,
            table_name,
            is_nullable,
            default_value,
            is_auto_increment,
            is_unique_key,
            is_primary_key,
            is_foreign_key,
        });
    }
    Ok(cols)
}

/// Parse result-info entries from the reader.
///
/// Wire format per entry:
/// ```text
/// [u8 stmt_type]
/// [i32 result_count]
/// [8 bytes OID]
/// [i32 cache_time_sec]    — skipped
/// [i32 cache_time_usec]   — skipped
/// ```
pub fn parse_result_infos(
    reader: &mut PacketReader<'_>,
    count: i32,
) -> Result<Vec<ResultInfo>, ProtocolError> {
    let mut infos = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let stmt_type_byte = reader.parse_byte()?;
        let stmt_type = StatementType::from_u8(stmt_type_byte);
        let result_count = reader.parse_int()?;
        let oid = reader.parse_raw_bytes(SIZE_OID)?;
        reader.parse_int()?; // cache_time_sec
        reader.parse_int()?; // cache_time_usec
        infos.push(ResultInfo {
            stmt_type,
            result_count,
            oid,
        });
    }
    Ok(infos)
}

/// Read a single typed column value from the reader.
///
/// `col_type` is the CUBRID data type; `size` is the byte count to read.
pub fn read_value(
    reader: &mut PacketReader<'_>,
    col_type: DataType,
    size: usize,
) -> Result<Value, ProtocolError> {
    match col_type {
        DataType::Char
        | DataType::String
        | DataType::NChar
        | DataType::VarNChar
        | DataType::Enum => {
            let s = reader.parse_null_term_string(size)?;
            Ok(Value::String(s))
        }
        DataType::Numeric => {
            // NUMERIC is transmitted as a null-terminated string
            let s = reader.parse_null_term_string(size)?;
            Ok(Value::String(s))
        }
        DataType::Short => {
            let v = reader.parse_short()?;
            Ok(Value::Short(v))
        }
        DataType::Int => {
            let v = reader.parse_int()?;
            Ok(Value::Int(v))
        }
        DataType::Bigint => {
            let v = reader.parse_long()?;
            Ok(Value::Long(v))
        }
        DataType::Float => {
            let v = reader.parse_float()?;
            Ok(Value::Float(v))
        }
        DataType::Double | DataType::Monetary => {
            let v = reader.parse_double()?;
            Ok(Value::Double(v))
        }
        DataType::Date => {
            let (year, month, day) = reader.parse_date()?;
            Ok(Value::Date { year, month, day })
        }
        DataType::Time => {
            let (hour, minute, second) = reader.parse_time()?;
            Ok(Value::Time {
                hour,
                minute,
                second,
            })
        }
        DataType::Datetime => {
            let (year, month, day, hour, minute, second, ms) = reader.parse_datetime()?;
            Ok(Value::Datetime {
                year,
                month,
                day,
                hour,
                minute,
                second,
                ms,
            })
        }
        DataType::Timestamp => {
            let (year, month, day, hour, minute, second) = reader.parse_timestamp()?;
            Ok(Value::Timestamp {
                year,
                month,
                day,
                hour,
                minute,
                second,
            })
        }
        DataType::Bit | DataType::VarBit | DataType::Blob | DataType::Clob => {
            let bytes = reader.parse_raw_bytes(size)?;
            Ok(Value::Bytes(bytes))
        }
        DataType::Null => Ok(Value::Null),
        DataType::Set | DataType::Multiset | DataType::Sequence | DataType::Object => {
            // Collections and objects — return raw bytes for now
            let bytes = reader.parse_raw_bytes(size)?;
            Ok(Value::Bytes(bytes))
        }
    }
}

/// Parse row data from the reader.
///
/// Wire format per row:
/// ```text
/// [i32 row_index]
/// [8 bytes OID]
/// for each column:
///   [i32 size]  — ≤ 0 means NULL
///   [value_bytes]
/// ```
///
/// For CALL/SP statement types or when the column type is NULL,
/// the first byte of the value is the actual type code.
pub fn parse_row_data(
    reader: &mut PacketReader<'_>,
    tuple_count: i32,
    columns: &[ColumnMetaData],
    stmt_type: StatementType,
) -> Result<Vec<Vec<Value>>, ProtocolError> {
    let is_call_type = stmt_type.is_call();
    let mut rows = Vec::with_capacity(tuple_count as usize);

    for _ in 0..tuple_count {
        reader.parse_int()?; // row index
        reader.skip(SIZE_OID)?; // OID

        let mut row = Vec::with_capacity(columns.len());
        for col in columns {
            let size = reader.parse_int()?;
            if size <= 0 {
                row.push(Value::Null);
                continue;
            }
            let mut actual_size = size as usize;
            let mut col_type = col.column_type;

            // For CALL/SP results or NULL-typed columns, the first byte is the type
            if is_call_type || col_type == DataType::Null {
                let type_byte = reader.parse_byte()?;
                actual_size -= 1;
                if actual_size == 0 {
                    row.push(Value::Null);
                    continue;
                }
                if let Some(dt) = DataType::from_u8(type_byte) {
                    col_type = dt;
                } else {
                    // Unknown type — read as raw bytes
                    let bytes = reader.parse_raw_bytes(actual_size)?;
                    row.push(Value::Bytes(bytes));
                    continue;
                }
            }

            row.push(read_value(reader, col_type, actual_size)?);
        }
        rows.push(row);
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Top-level response parsers
// ---------------------------------------------------------------------------

/// Parse a `PrepareAndExecute` (FC=41) response.
///
/// `data` should start with `CAS_INFO` (4 bytes) followed by the response body.
pub fn parse_prepare_and_execute(
    data: &[u8],
    proto_version: i32,
) -> Result<(PrepareAndExecuteResult, [u8; SIZE_CAS_INFO]), ProtocolError> {
    let mut reader = PacketReader::new(data);
    let cas_info = reader.parse_cas_info()?;

    let response_code = reader.parse_int()?;
    if response_code < 0 {
        let (code, message) = reader.read_error(reader.remaining())?;
        return Err(ProtocolError::ServerError { code, message });
    }

    let query_handle = response_code;
    reader.parse_int()?; // result cache lifetime
    let stmt_type_byte = reader.parse_byte()?;
    let statement_type = StatementType::from_u8(stmt_type_byte);
    let bind_count = reader.parse_int()?;
    reader.parse_byte()?; // is_updatable
    let col_count = reader.parse_int()?;
    let columns = parse_column_metadata(&mut reader, col_count)?;

    let total_tuple_count = reader.parse_int()?;
    reader.parse_byte()?; // cache_reusable
    let result_count = reader.parse_int()?;
    let result_infos = parse_result_infos(&mut reader, result_count)?;

    if proto_version > 1 {
        reader.parse_byte()?; // includes_column_info
    }
    if proto_version > 4 {
        reader.parse_int()?; // shard_id
    }

    let mut tuple_count = 0;
    let mut rows = Vec::new();

    if statement_type.is_select() && reader.remaining() >= SIZE_INT * 2 {
        reader.parse_int()?; // fetch_code
        tuple_count = reader.parse_int()?;
        if tuple_count > 0 {
            rows = parse_row_data(&mut reader, tuple_count, &columns, statement_type)?;
        }
    }

    Ok((
        PrepareAndExecuteResult {
            query_handle,
            statement_type,
            bind_count,
            columns,
            total_tuple_count,
            result_infos,
            tuple_count,
            rows,
        },
        cas_info,
    ))
}

/// Parse a `Prepare` (FC=2) response.
pub fn parse_prepare(data: &[u8]) -> Result<(PrepareResult, [u8; SIZE_CAS_INFO]), ProtocolError> {
    let mut reader = PacketReader::new(data);
    let cas_info = reader.parse_cas_info()?;

    let response_code = reader.parse_int()?;
    if response_code < 0 {
        let (code, message) = reader.read_error(reader.remaining())?;
        return Err(ProtocolError::ServerError { code, message });
    }

    let query_handle = response_code;
    reader.parse_int()?; // result cache lifetime
    let stmt_type_byte = reader.parse_byte()?;
    let statement_type = StatementType::from_u8(stmt_type_byte);
    let bind_count = reader.parse_int()?;
    reader.parse_byte()?; // is_updatable
    let col_count = reader.parse_int()?;
    let columns = parse_column_metadata(&mut reader, col_count)?;

    Ok((
        PrepareResult {
            query_handle,
            statement_type,
            bind_count,
            columns,
        },
        cas_info,
    ))
}

/// Parse an `Execute` (FC=3) response.
pub fn parse_execute(
    data: &[u8],
    columns: &[ColumnMetaData],
    stmt_type: StatementType,
    proto_version: i32,
) -> Result<(ExecuteResult, [u8; SIZE_CAS_INFO]), ProtocolError> {
    let mut reader = PacketReader::new(data);
    let cas_info = reader.parse_cas_info()?;

    let response_code = reader.parse_int()?;
    if response_code < 0 {
        let (code, message) = reader.read_error(reader.remaining())?;
        return Err(ProtocolError::ServerError { code, message });
    }

    let total_tuple_count = response_code;
    reader.parse_byte()?; // cache_reusable
    let result_count = reader.parse_int()?;
    let result_infos = parse_result_infos(&mut reader, result_count)?;

    if proto_version > 1 {
        reader.parse_byte()?; // includes_column_info
    }
    if proto_version > 4 {
        reader.parse_int()?; // shard_id
    }

    let mut tuple_count = 0;
    let mut rows = Vec::new();

    if stmt_type.is_select() && reader.remaining() >= SIZE_INT * 2 {
        reader.parse_int()?; // fetch_code
        tuple_count = reader.parse_int()?;
        if tuple_count > 0 && !columns.is_empty() {
            rows = parse_row_data(&mut reader, tuple_count, columns, stmt_type)?;
        }
    }

    Ok((
        ExecuteResult {
            total_tuple_count,
            result_infos,
            tuple_count,
            rows,
        },
        cas_info,
    ))
}

/// Parse a `Fetch` (FC=8) response.
pub fn parse_fetch(
    data: &[u8],
    columns: &[ColumnMetaData],
    stmt_type: StatementType,
) -> Result<(FetchResult, [u8; SIZE_CAS_INFO]), ProtocolError> {
    let mut reader = PacketReader::new(data);
    let cas_info = reader.parse_cas_info()?;

    let response_code = reader.parse_int()?;
    if response_code < 0 {
        let (code, message) = reader.read_error(reader.remaining())?;
        return Err(ProtocolError::ServerError { code, message });
    }

    let tuple_count = reader.parse_int()?;
    let mut rows = Vec::new();
    if tuple_count > 0 && !columns.is_empty() {
        rows = parse_row_data(&mut reader, tuple_count, columns, stmt_type)?;
    }

    Ok((FetchResult { tuple_count, rows }, cas_info))
}

/// Parse a `GetLastInsertId` (FC=40) response.
pub fn parse_get_last_insert_id(
    data: &[u8],
) -> Result<(String, [u8; SIZE_CAS_INFO]), ProtocolError> {
    let mut reader = PacketReader::new(data);
    let cas_info = reader.parse_cas_info()?;

    let code = reader.parse_int()?;
    if code < 0 {
        let (err_code, message) = reader.read_error(reader.remaining())?;
        return Err(ProtocolError::ServerError {
            code: err_code,
            message,
        });
    }

    let id = if code > 0 {
        reader.parse_null_term_string(code as usize)?
    } else {
        String::new()
    };

    Ok((id, cas_info))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal column metadata blob for one column.
    fn build_column_meta(col_type: u8, name: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        // legacy type (no 0x80 flag)
        buf.push(col_type);
        // scale: i16
        buf.extend_from_slice(&0i16.to_be_bytes());
        // precision: i32
        buf.extend_from_slice(&10i32.to_be_bytes());
        // name
        let name_bytes = name.as_bytes();
        buf.extend_from_slice(&((name_bytes.len() + 1) as i32).to_be_bytes());
        buf.extend_from_slice(name_bytes);
        buf.push(0x00);
        // real_name (same as name)
        buf.extend_from_slice(&((name_bytes.len() + 1) as i32).to_be_bytes());
        buf.extend_from_slice(name_bytes);
        buf.push(0x00);
        // table_name
        let tbl = b"test_table";
        buf.extend_from_slice(&((tbl.len() + 1) as i32).to_be_bytes());
        buf.extend_from_slice(tbl);
        buf.push(0x00);
        // is_nullable
        buf.push(1);
        // default_value (empty)
        buf.extend_from_slice(&1i32.to_be_bytes());
        buf.push(0x00);
        // is_auto_increment
        buf.push(0);
        // is_unique_key
        buf.push(0);
        // is_primary_key
        buf.push(1);
        // is_reverse_index
        buf.push(0);
        // is_reverse_unique
        buf.push(0);
        // is_foreign_key
        buf.push(0);
        // is_shared
        buf.push(0);
        buf
    }

    /// Helper: build a result info blob for one result.
    fn build_result_info(stmt_type: u8, result_count: i32) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(stmt_type);
        buf.extend_from_slice(&result_count.to_be_bytes());
        buf.extend_from_slice(&[0u8; SIZE_OID]); // OID
        buf.extend_from_slice(&0i32.to_be_bytes()); // cache_time_sec
        buf.extend_from_slice(&0i32.to_be_bytes()); // cache_time_usec
        buf
    }

    #[test]
    fn test_parse_column_metadata_single() {
        let meta_bytes = build_column_meta(DataType::Int as u8, "id");
        let mut reader = PacketReader::new(&meta_bytes);
        let cols = parse_column_metadata(&mut reader, 1).unwrap();
        assert_eq!(cols.len(), 1);
        assert_eq!(cols[0].column_type, DataType::Int);
        assert_eq!(cols[0].name, "id");
        assert_eq!(cols[0].real_name, "id");
        assert_eq!(cols[0].table_name, "test_table");
        assert!(cols[0].is_nullable);
        assert!(cols[0].is_primary_key);
        assert!(!cols[0].is_auto_increment);
    }

    #[test]
    fn test_parse_column_metadata_legacy_type() {
        let mut meta_bytes = Vec::new();
        // legacy type with 0x80 flag — actual type follows
        meta_bytes.push(0x80 | 0x01); // flag set, junk bits
        meta_bytes.push(DataType::String as u8); // actual type
                                                 // scale
        meta_bytes.extend_from_slice(&0i16.to_be_bytes());
        // precision
        meta_bytes.extend_from_slice(&255i32.to_be_bytes());
        // name
        let name = b"col1";
        meta_bytes.extend_from_slice(&((name.len() + 1) as i32).to_be_bytes());
        meta_bytes.extend_from_slice(name);
        meta_bytes.push(0x00);
        // real_name
        meta_bytes.extend_from_slice(&((name.len() + 1) as i32).to_be_bytes());
        meta_bytes.extend_from_slice(name);
        meta_bytes.push(0x00);
        // table_name
        let tbl = b"t";
        meta_bytes.extend_from_slice(&((tbl.len() + 1) as i32).to_be_bytes());
        meta_bytes.extend_from_slice(tbl);
        meta_bytes.push(0x00);
        // flags
        meta_bytes.push(0); // nullable
        meta_bytes.extend_from_slice(&1i32.to_be_bytes()); // default len
        meta_bytes.push(0x00); // default value
        meta_bytes.push(0); // auto_increment
        meta_bytes.push(0); // unique
        meta_bytes.push(0); // primary
        meta_bytes.push(0); // reverse_index
        meta_bytes.push(0); // reverse_unique
        meta_bytes.push(0); // foreign
        meta_bytes.push(0); // shared

        let mut reader = PacketReader::new(&meta_bytes);
        let cols = parse_column_metadata(&mut reader, 1).unwrap();
        assert_eq!(cols[0].column_type, DataType::String);
        assert_eq!(cols[0].precision, 255);
    }

    #[test]
    fn test_parse_result_infos_single() {
        let info_bytes = build_result_info(StatementType::Insert as u8, 5);
        let mut reader = PacketReader::new(&info_bytes);
        let infos = parse_result_infos(&mut reader, 1).unwrap();
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].stmt_type, StatementType::Insert);
        assert_eq!(infos[0].result_count, 5);
        assert_eq!(infos[0].oid.len(), SIZE_OID);
    }

    #[test]
    fn test_read_value_int() {
        let data = 42i32.to_be_bytes();
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::Int, 4).unwrap();
        assert_eq!(v, Value::Int(42));
    }

    #[test]
    fn test_read_value_string() {
        let mut data = Vec::new();
        data.extend_from_slice(b"hello");
        data.push(0x00);
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::String, 6).unwrap();
        assert_eq!(v, Value::String("hello".to_string()));
    }

    #[test]
    fn test_read_value_double() {
        let data = std::f64::consts::PI.to_be_bytes();
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::Double, 8).unwrap();
        assert_eq!(v, Value::Double(std::f64::consts::PI));
    }

    #[test]
    fn test_read_value_date() {
        let mut data = Vec::new();
        data.extend_from_slice(&2024i16.to_be_bytes());
        data.extend_from_slice(&3i16.to_be_bytes());
        data.extend_from_slice(&15i16.to_be_bytes());
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::Date, 6).unwrap();
        assert_eq!(
            v,
            Value::Date {
                year: 2024,
                month: 3,
                day: 15
            }
        );
    }

    #[test]
    fn test_read_value_time() {
        let mut data = Vec::new();
        data.extend_from_slice(&10i16.to_be_bytes());
        data.extend_from_slice(&30i16.to_be_bytes());
        data.extend_from_slice(&45i16.to_be_bytes());
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::Time, 6).unwrap();
        assert_eq!(
            v,
            Value::Time {
                hour: 10,
                minute: 30,
                second: 45
            }
        );
    }

    #[test]
    fn test_read_value_timestamp() {
        let mut data = Vec::new();
        for v in [2024i16, 3, 15, 10, 30, 45] {
            data.extend_from_slice(&v.to_be_bytes());
        }
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::Timestamp, 12).unwrap();
        assert_eq!(
            v,
            Value::Timestamp {
                year: 2024,
                month: 3,
                day: 15,
                hour: 10,
                minute: 30,
                second: 45
            }
        );
    }

    #[test]
    fn test_read_value_datetime() {
        let mut data = Vec::new();
        for v in [2024i16, 3, 15, 10, 30, 45, 123] {
            data.extend_from_slice(&v.to_be_bytes());
        }
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::Datetime, 14).unwrap();
        assert_eq!(
            v,
            Value::Datetime {
                year: 2024,
                month: 3,
                day: 15,
                hour: 10,
                minute: 30,
                second: 45,
                ms: 123
            }
        );
    }

    #[test]
    fn test_read_value_bytes() {
        let data = [0xDE, 0xAD, 0xBE, 0xEF];
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::Bit, 4).unwrap();
        assert_eq!(v, Value::Bytes(vec![0xDE, 0xAD, 0xBE, 0xEF]));
    }

    #[test]
    fn test_read_value_null() {
        let data: [u8; 0] = [];
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::Null, 0).unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn test_read_value_short() {
        let data = 42i16.to_be_bytes();
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::Short, 2).unwrap();
        assert_eq!(v, Value::Short(42));
    }

    #[test]
    fn test_read_value_bigint() {
        let data = 1234567890i64.to_be_bytes();
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::Bigint, 8).unwrap();
        assert_eq!(v, Value::Long(1234567890));
    }

    #[test]
    fn test_read_value_float() {
        let data = 2.5f32.to_be_bytes();
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::Float, 4).unwrap();
        assert_eq!(v, Value::Float(2.5));
    }

    #[test]
    fn test_read_value_monetary() {
        let data = 99.99f64.to_be_bytes();
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::Monetary, 8).unwrap();
        assert_eq!(v, Value::Double(99.99));
    }

    #[test]
    fn test_read_value_numeric() {
        let mut data = Vec::new();
        data.extend_from_slice(b"123.45");
        data.push(0x00);
        let mut reader = PacketReader::new(&data);
        let v = read_value(&mut reader, DataType::Numeric, 7).unwrap();
        assert_eq!(v, Value::String("123.45".to_string()));
    }

    #[test]
    fn test_parse_row_data_single_row() {
        let col = ColumnMetaData {
            column_type: DataType::Int,
            scale: 0,
            precision: 10,
            name: "id".to_string(),
            real_name: "id".to_string(),
            table_name: "t".to_string(),
            is_nullable: false,
            default_value: String::new(),
            is_auto_increment: false,
            is_unique_key: false,
            is_primary_key: false,
            is_foreign_key: false,
        };

        let mut data = Vec::new();
        // row_index
        data.extend_from_slice(&1i32.to_be_bytes());
        // OID (8 bytes)
        data.extend_from_slice(&[0u8; SIZE_OID]);
        // column 1: size=4, value=42
        data.extend_from_slice(&4i32.to_be_bytes());
        data.extend_from_slice(&42i32.to_be_bytes());

        let mut reader = PacketReader::new(&data);
        let rows = parse_row_data(&mut reader, 1, &[col], StatementType::Select).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 1);
        assert_eq!(rows[0][0], Value::Int(42));
    }

    #[test]
    fn test_parse_row_data_null_column() {
        let col = ColumnMetaData {
            column_type: DataType::String,
            scale: 0,
            precision: 255,
            name: "name".to_string(),
            real_name: "name".to_string(),
            table_name: "t".to_string(),
            is_nullable: true,
            default_value: String::new(),
            is_auto_increment: false,
            is_unique_key: false,
            is_primary_key: false,
            is_foreign_key: false,
        };

        let mut data = Vec::new();
        // row_index
        data.extend_from_slice(&1i32.to_be_bytes());
        // OID
        data.extend_from_slice(&[0u8; SIZE_OID]);
        // column 1: size=0 (NULL)
        data.extend_from_slice(&0i32.to_be_bytes());

        let mut reader = PacketReader::new(&data);
        let rows = parse_row_data(&mut reader, 1, &[col], StatementType::Select).unwrap();
        assert_eq!(rows[0][0], Value::Null);
    }

    #[test]
    fn test_parse_row_data_multiple_columns() {
        let cols = vec![
            ColumnMetaData {
                column_type: DataType::Int,
                scale: 0,
                precision: 10,
                name: "id".to_string(),
                real_name: "id".to_string(),
                table_name: "t".to_string(),
                is_nullable: false,
                default_value: String::new(),
                is_auto_increment: false,
                is_unique_key: false,
                is_primary_key: false,
                is_foreign_key: false,
            },
            ColumnMetaData {
                column_type: DataType::String,
                scale: 0,
                precision: 255,
                name: "name".to_string(),
                real_name: "name".to_string(),
                table_name: "t".to_string(),
                is_nullable: true,
                default_value: String::new(),
                is_auto_increment: false,
                is_unique_key: false,
                is_primary_key: false,
                is_foreign_key: false,
            },
        ];

        let mut data = Vec::new();
        // row_index
        data.extend_from_slice(&1i32.to_be_bytes());
        // OID
        data.extend_from_slice(&[0u8; SIZE_OID]);
        // column 1: Int(42)
        data.extend_from_slice(&4i32.to_be_bytes());
        data.extend_from_slice(&42i32.to_be_bytes());
        // column 2: String("hello")
        data.extend_from_slice(&6i32.to_be_bytes());
        data.extend_from_slice(b"hello\0");

        let mut reader = PacketReader::new(&data);
        let rows = parse_row_data(&mut reader, 1, &cols, StatementType::Select).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], Value::Int(42));
        assert_eq!(rows[0][1], Value::String("hello".to_string()));
    }

    #[test]
    fn test_parse_prepare_success() {
        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // response code = query handle (positive = success)
        data.extend_from_slice(&10i32.to_be_bytes());
        // result cache lifetime
        data.extend_from_slice(&0i32.to_be_bytes());
        // statement type (SELECT = 21)
        data.push(StatementType::Select as u8);
        // bind count
        data.extend_from_slice(&2i32.to_be_bytes());
        // is_updatable
        data.push(0);
        // column count = 0
        data.extend_from_slice(&0i32.to_be_bytes());

        let (result, cas_info) = parse_prepare(&data).unwrap();
        assert_eq!(cas_info, [0x01, 0x02, 0x03, 0x04]);
        assert_eq!(result.query_handle, 10);
        assert_eq!(result.statement_type, StatementType::Select);
        assert_eq!(result.bind_count, 2);
        assert!(result.columns.is_empty());
    }

    #[test]
    fn test_parse_prepare_error() {
        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // negative response code = error
        data.extend_from_slice(&(-1i32).to_be_bytes());
        // error code
        data.extend_from_slice(&(-200i32).to_be_bytes());
        // error message
        data.extend_from_slice(b"syntax error\0");

        let err = parse_prepare(&data).unwrap_err();
        match err {
            ProtocolError::ServerError { code, message } => {
                assert_eq!(code, -200);
                assert_eq!(message, "syntax error");
            }
            other => panic!("expected ServerError, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_execute_insert() {
        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);
        // response code = total_tuple_count = 1
        data.extend_from_slice(&1i32.to_be_bytes());
        // cache_reusable
        data.push(0);
        // result count = 1
        data.extend_from_slice(&1i32.to_be_bytes());
        // result info: INSERT, count=1
        data.extend_from_slice(&build_result_info(StatementType::Insert as u8, 1));
        // includes_column_info (proto_version > 1)
        data.push(0);
        // shard_id (proto_version > 4)
        data.extend_from_slice(&0i32.to_be_bytes());

        let (result, cas_info) = parse_execute(&data, &[], StatementType::Insert, 7).unwrap();
        assert_eq!(cas_info, [0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(result.total_tuple_count, 1);
        assert_eq!(result.result_infos.len(), 1);
        assert_eq!(result.result_infos[0].stmt_type, StatementType::Insert);
        assert!(result.rows.is_empty());
    }

    #[test]
    fn test_parse_fetch_success() {
        let col = ColumnMetaData {
            column_type: DataType::Int,
            scale: 0,
            precision: 10,
            name: "id".to_string(),
            real_name: "id".to_string(),
            table_name: "t".to_string(),
            is_nullable: false,
            default_value: String::new(),
            is_auto_increment: false,
            is_unique_key: false,
            is_primary_key: false,
            is_foreign_key: false,
        };

        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // response code (positive)
        data.extend_from_slice(&0i32.to_be_bytes());
        // tuple_count = 1
        data.extend_from_slice(&1i32.to_be_bytes());
        // row data
        data.extend_from_slice(&1i32.to_be_bytes()); // row_index
        data.extend_from_slice(&[0u8; SIZE_OID]); // OID
        data.extend_from_slice(&4i32.to_be_bytes()); // col size
        data.extend_from_slice(&99i32.to_be_bytes()); // value

        let (result, _) = parse_fetch(&data, &[col], StatementType::Select).unwrap();
        assert_eq!(result.tuple_count, 1);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], Value::Int(99));
    }

    #[test]
    fn test_parse_fetch_empty() {
        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // response code
        data.extend_from_slice(&0i32.to_be_bytes());
        // tuple_count = 0
        data.extend_from_slice(&0i32.to_be_bytes());

        let (result, _) = parse_fetch(&data, &[], StatementType::Select).unwrap();
        assert_eq!(result.tuple_count, 0);
        assert!(result.rows.is_empty());
    }

    #[test]
    fn test_parse_fetch_error() {
        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // negative response
        data.extend_from_slice(&(-1i32).to_be_bytes());
        // error details
        data.extend_from_slice(&(-500i32).to_be_bytes());
        data.extend_from_slice(b"fetch error\0");

        let err = parse_fetch(&data, &[], StatementType::Select).unwrap_err();
        match err {
            ProtocolError::ServerError { code, message } => {
                assert_eq!(code, -500);
                assert_eq!(message, "fetch error");
            }
            other => panic!("expected ServerError, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_get_last_insert_id_with_value() {
        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // code > 0 = length of ID string
        let id_str = b"42\0";
        data.extend_from_slice(&(id_str.len() as i32).to_be_bytes());
        data.extend_from_slice(id_str);

        let (id, cas_info) = parse_get_last_insert_id(&data).unwrap();
        assert_eq!(id, "42");
        assert_eq!(cas_info, [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_parse_get_last_insert_id_zero() {
        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // code = 0 = no last insert ID
        data.extend_from_slice(&0i32.to_be_bytes());

        let (id, _) = parse_get_last_insert_id(&data).unwrap();
        assert_eq!(id, "");
    }

    #[test]
    fn test_parse_get_last_insert_id_error() {
        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // negative code
        data.extend_from_slice(&(-1i32).to_be_bytes());
        // error
        data.extend_from_slice(&(-300i32).to_be_bytes());
        data.extend_from_slice(b"no insert\0");

        let err = parse_get_last_insert_id(&data).unwrap_err();
        match err {
            ProtocolError::ServerError { code, message } => {
                assert_eq!(code, -300);
                assert_eq!(message, "no insert");
            }
            other => panic!("expected ServerError, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_prepare_and_execute_insert_no_rows() {
        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // response code = query handle
        data.extend_from_slice(&5i32.to_be_bytes());
        // result cache lifetime
        data.extend_from_slice(&0i32.to_be_bytes());
        // statement type (INSERT = 20)
        data.push(StatementType::Insert as u8);
        // bind count
        data.extend_from_slice(&0i32.to_be_bytes());
        // is_updatable
        data.push(0);
        // column count = 0
        data.extend_from_slice(&0i32.to_be_bytes());
        // total_tuple_count
        data.extend_from_slice(&1i32.to_be_bytes());
        // cache_reusable
        data.push(0);
        // result count = 1
        data.extend_from_slice(&1i32.to_be_bytes());
        // result info
        data.extend_from_slice(&build_result_info(StatementType::Insert as u8, 1));
        // proto_version > 1: includes_column_info
        data.push(0);

        let (result, cas_info) = parse_prepare_and_execute(&data, 3).unwrap();
        assert_eq!(cas_info, [0x01, 0x02, 0x03, 0x04]);
        assert_eq!(result.query_handle, 5);
        assert_eq!(result.statement_type, StatementType::Insert);
        assert_eq!(result.total_tuple_count, 1);
        assert_eq!(result.result_infos.len(), 1);
        assert!(result.rows.is_empty());
    }

    #[test]
    fn test_parse_prepare_and_execute_error() {
        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // negative response
        data.extend_from_slice(&(-1i32).to_be_bytes());
        // error code + message
        data.extend_from_slice(&(-100i32).to_be_bytes());
        data.extend_from_slice(b"bad sql\0");

        let err = parse_prepare_and_execute(&data, 7).unwrap_err();
        match err {
            ProtocolError::ServerError { code, message } => {
                assert_eq!(code, -100);
                assert_eq!(message, "bad sql");
            }
            other => panic!("expected ServerError, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_prepare_and_execute_select_with_rows() {
        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // response code = query handle
        data.extend_from_slice(&7i32.to_be_bytes());
        // result cache lifetime
        data.extend_from_slice(&0i32.to_be_bytes());
        // statement type (SELECT = 21)
        data.push(StatementType::Select as u8);
        // bind count
        data.extend_from_slice(&0i32.to_be_bytes());
        // is_updatable
        data.push(0);
        // column count = 1
        data.extend_from_slice(&1i32.to_be_bytes());
        // column metadata
        data.extend_from_slice(&build_column_meta(DataType::Int as u8, "val"));
        // total_tuple_count
        data.extend_from_slice(&1i32.to_be_bytes());
        // cache_reusable
        data.push(0);
        // result count = 1
        data.extend_from_slice(&1i32.to_be_bytes());
        // result info
        data.extend_from_slice(&build_result_info(StatementType::Select as u8, 1));

        // proto_version=1, no includes_column_info or shard_id

        // inline fetch data for SELECT
        // fetch_code
        data.extend_from_slice(&0i32.to_be_bytes());
        // tuple_count = 1
        data.extend_from_slice(&1i32.to_be_bytes());
        // row: index + OID + col(size=4, val=99)
        data.extend_from_slice(&1i32.to_be_bytes());
        data.extend_from_slice(&[0u8; SIZE_OID]);
        data.extend_from_slice(&4i32.to_be_bytes());
        data.extend_from_slice(&99i32.to_be_bytes());

        let (result, _) = parse_prepare_and_execute(&data, 1).unwrap();
        assert_eq!(result.query_handle, 7);
        assert_eq!(result.statement_type, StatementType::Select);
        assert_eq!(result.columns.len(), 1);
        assert_eq!(result.columns[0].name, "val");
        assert_eq!(result.tuple_count, 1);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], Value::Int(99));
    }

    #[test]
    fn test_parse_execute_select_with_rows() {
        let col = ColumnMetaData {
            column_type: DataType::String,
            scale: 0,
            precision: 255,
            name: "name".to_string(),
            real_name: "name".to_string(),
            table_name: "t".to_string(),
            is_nullable: true,
            default_value: String::new(),
            is_auto_increment: false,
            is_unique_key: false,
            is_primary_key: false,
            is_foreign_key: false,
        };

        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // response code = total_tuple_count
        data.extend_from_slice(&1i32.to_be_bytes());
        // cache_reusable
        data.push(0);
        // result count = 1
        data.extend_from_slice(&1i32.to_be_bytes());
        // result info
        data.extend_from_slice(&build_result_info(StatementType::Select as u8, 1));

        // proto_version=1, no extras

        // inline fetch for SELECT
        data.extend_from_slice(&0i32.to_be_bytes()); // fetch_code
        data.extend_from_slice(&1i32.to_be_bytes()); // tuple_count

        // row data
        data.extend_from_slice(&1i32.to_be_bytes()); // row_index
        data.extend_from_slice(&[0u8; SIZE_OID]); // OID
        let val = b"world\0";
        data.extend_from_slice(&(val.len() as i32).to_be_bytes());
        data.extend_from_slice(val);

        let (result, _) = parse_execute(&data, &[col], StatementType::Select, 1).unwrap();
        assert_eq!(result.total_tuple_count, 1);
        assert_eq!(result.tuple_count, 1);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0][0], Value::String("world".to_string()));
    }

    #[test]
    fn test_read_value_set_type() {
        // Set, Multiset, Sequence, Object types should return raw bytes
        let set_data: Vec<u8> = vec![0x01, 0x02, 0x03, 0x04];
        let mut reader = PacketReader::new(&set_data);
        let val = read_value(&mut reader, DataType::Set, 4).unwrap();
        assert_eq!(val, Value::Bytes(vec![0x01, 0x02, 0x03, 0x04]));
    }

    #[test]
    fn test_read_value_multiset_type() {
        let data: Vec<u8> = vec![0xAA, 0xBB];
        let mut reader = PacketReader::new(&data);
        let val = read_value(&mut reader, DataType::Multiset, 2).unwrap();
        assert_eq!(val, Value::Bytes(vec![0xAA, 0xBB]));
    }

    #[test]
    fn test_read_value_sequence_type() {
        let data: Vec<u8> = vec![0xCC];
        let mut reader = PacketReader::new(&data);
        let val = read_value(&mut reader, DataType::Sequence, 1).unwrap();
        assert_eq!(val, Value::Bytes(vec![0xCC]));
    }

    #[test]
    fn test_read_value_object_type() {
        let data: Vec<u8> = vec![0x10, 0x20, 0x30];
        let mut reader = PacketReader::new(&data);
        let val = read_value(&mut reader, DataType::Object, 3).unwrap();
        assert_eq!(val, Value::Bytes(vec![0x10, 0x20, 0x30]));
    }

    #[test]
    fn test_parse_row_data_null_typed_column() {
        // Test CALL/SP result type detection: column with Null type gets type from first byte
        let col = ColumnMetaData {
            column_type: DataType::Null,
            scale: 0,
            precision: 255,
            name: "result".to_string(),
            real_name: "result".to_string(),
            table_name: "".to_string(),
            is_nullable: true,
            default_value: String::new(),
            is_auto_increment: false,
            is_unique_key: false,
            is_primary_key: false,
            is_foreign_key: false,
        };

        let mut data = Vec::new();
        // row_index
        data.extend_from_slice(&1i32.to_be_bytes());
        // OID
        data.extend_from_slice(&[0u8; SIZE_OID]);
        // column size = 5 (1 byte type + 4 bytes int)
        data.extend_from_slice(&5i32.to_be_bytes());
        // type byte = Int (4)
        data.push(DataType::Int as u8);
        // actual value: 42
        data.extend_from_slice(&42i32.to_be_bytes());

        let mut reader = PacketReader::new(&data);
        let rows = parse_row_data(&mut reader, 1, &[col], StatementType::Call).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], Value::Int(42));
    }

    #[test]
    fn test_parse_row_data_null_typed_column_null_value() {
        // Null-typed column with size=1 (type byte only, no data) → Null
        let col = ColumnMetaData {
            column_type: DataType::Null,
            scale: 0,
            precision: 255,
            name: "result".to_string(),
            real_name: "result".to_string(),
            table_name: "".to_string(),
            is_nullable: true,
            default_value: String::new(),
            is_auto_increment: false,
            is_unique_key: false,
            is_primary_key: false,
            is_foreign_key: false,
        };

        let mut data = Vec::new();
        // row_index
        data.extend_from_slice(&1i32.to_be_bytes());
        // OID
        data.extend_from_slice(&[0u8; SIZE_OID]);
        // column size = 1 (type byte only)
        data.extend_from_slice(&1i32.to_be_bytes());
        // type byte = Int, but actual_size after subtracting type byte = 0
        data.push(DataType::Int as u8);

        let mut reader = PacketReader::new(&data);
        let rows = parse_row_data(&mut reader, 1, &[col], StatementType::Call).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], Value::Null);
    }

    #[test]
    fn test_parse_row_data_null_typed_column_unknown_type() {
        // Null-typed column with unknown type byte → raw bytes
        let col = ColumnMetaData {
            column_type: DataType::Null,
            scale: 0,
            precision: 255,
            name: "result".to_string(),
            real_name: "result".to_string(),
            table_name: "".to_string(),
            is_nullable: true,
            default_value: String::new(),
            is_auto_increment: false,
            is_unique_key: false,
            is_primary_key: false,
            is_foreign_key: false,
        };

        let mut data = Vec::new();
        // row_index
        data.extend_from_slice(&1i32.to_be_bytes());
        // OID
        data.extend_from_slice(&[0u8; SIZE_OID]);
        // column size = 5 (1 byte type + 4 bytes data)
        data.extend_from_slice(&5i32.to_be_bytes());
        // type byte = 0xFF (unknown)
        data.push(0xFF);
        // raw data
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);

        let mut reader = PacketReader::new(&data);
        let rows = parse_row_data(&mut reader, 1, &[col], StatementType::Call).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0], Value::Bytes(vec![0x01, 0x02, 0x03, 0x04]));
    }

    #[test]
    fn test_parse_execute_error() {
        let mut data = Vec::new();
        // CAS_INFO
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // negative response code = error
        data.extend_from_slice(&(-1i32).to_be_bytes());
        // error code + message
        data.extend_from_slice(&(-500i32).to_be_bytes());
        data.extend_from_slice(b"execute failed\0");

        let err = parse_execute(&data, &[], StatementType::Select, 1).unwrap_err();
        match err {
            ProtocolError::ServerError { code, message } => {
                assert_eq!(code, -500);
                assert_eq!(message, "execute failed");
            }
            other => panic!("expected ServerError, got {other:?}"),
        }
    }
}
