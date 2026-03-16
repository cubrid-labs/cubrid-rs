//! Broker handshake and database authentication.
//!
//! ## Connection flow
//!
//! 1. Connect to broker at `host:port`
//! 2. Send 10-byte `ClientInfoExchange` (unframed)
//! 3. Receive 4-byte redirect port (big-endian i32)
//!    - `< 0`: rejected
//!    - `= 0`: reuse same socket
//!    - `> 0`: connect to new CAS port
//! 4. Send 628-byte `OpenDatabase` request (unframed)
//! 5. Receive framed `OpenDatabase` response

use crate::codec::{PacketReader, PacketWriter};
use crate::constants::*;
use crate::error::ProtocolError;

/// Build the 10-byte ClientInfoExchange handshake request.
///
/// Wire format (unframed, no header):
/// ```text
/// [0..4]  "CUBRK" (5 bytes)
/// [5]     CLIENT_TYPE_JDBC (0x03)
/// [6]     CAS_VERSION (0x47)
/// [7..9]  padding (3 × 0x00)
/// ```
pub fn write_client_info_exchange() -> [u8; CLIENT_INFO_EXCHANGE_SIZE] {
    let mut buf = [0u8; CLIENT_INFO_EXCHANGE_SIZE];
    buf[..5].copy_from_slice(BROKER_MAGIC);
    buf[5] = CLIENT_TYPE_JDBC;
    buf[6] = CAS_VERSION;
    // [7..9] are already zero (padding)
    buf
}

/// Parse the 4-byte broker handshake response to get the redirect port.
///
/// Returns:
/// - `< 0`: connection rejected (error)
/// - `= 0`: reuse current socket for CAS
/// - `> 0`: connect to this new port for CAS
pub fn parse_client_info_exchange(data: &[u8; BROKER_RESPONSE_SIZE]) -> i32 {
    i32::from_be_bytes(*data)
}

/// Build the 628-byte OpenDatabase request (unframed).
///
/// Wire format:
/// ```text
/// [0..31]    database name (32 bytes, fixed, zero-padded)
/// [32..63]   user name (32 bytes, fixed, zero-padded)
/// [64..95]   password (32 bytes, fixed, zero-padded)
/// [96..607]  extended info filler (512 bytes, all zeros)
/// [608..627] reserved filler (20 bytes, all zeros)
/// ```
pub fn write_open_database(database: &str, user: &str, password: &str) -> Vec<u8> {
    let mut w = PacketWriter::with_capacity(DB_OPEN_PAYLOAD_SIZE);
    w.write_fixed_string(database, DB_NAME_SIZE);
    w.write_fixed_string(user, DB_USER_SIZE);
    w.write_fixed_string(password, DB_PASSWORD_SIZE);
    w.write_filler(DB_EXTENDED_SIZE);
    w.write_filler(DB_RESERVED_SIZE);
    debug_assert_eq!(w.len(), DB_OPEN_PAYLOAD_SIZE);
    w.into_bytes()
}

/// Result of a successful OpenDatabase handshake.
#[derive(Debug, Clone)]
pub struct OpenDatabaseResult {
    /// CAS info bytes (opaque 4-byte token for subsequent requests).
    pub cas_info: [u8; SIZE_CAS_INFO],
    /// Protocol version negotiated with the server.
    pub proto_version: i32,
    /// Session ID assigned by the server.
    pub session_id: i32,
}

/// Parse the OpenDatabase response.
///
/// The response is framed: the caller should read `DATA_LENGTH` first,
/// then read `DATA_LENGTH + SIZE_CAS_INFO` bytes. Pass that buffer here.
///
/// The buffer starts with CAS_INFO (4 bytes), then:
/// ```text
/// [i32] responseCode (< 0 = error)
/// [16 bytes] broker info
/// [i32] session ID
/// ```
pub fn parse_open_database(data: &[u8]) -> Result<OpenDatabaseResult, ProtocolError> {
    let mut reader = PacketReader::new(data);

    // First 4 bytes: CAS_INFO
    let cas_info = reader.parse_cas_info()?;

    // Response code
    let response_code = reader.parse_int()?;
    if response_code < 0 {
        let (code, message) = reader.read_error(reader.remaining())?;
        return Err(ProtocolError::ServerError { code, message });
    }

    // Broker info (16 bytes)
    let broker_bytes = reader.parse_raw_bytes(SIZE_BROKER_INFO)?;
    let proto_version = (broker_bytes[4] & 0x3F) as i32;

    // Session ID
    let session_id = reader.parse_int()?;

    Ok(OpenDatabaseResult {
        cas_info,
        proto_version,
        session_id,
    })
}

/// Build a ConClose request (FC=31).
pub fn write_con_close(cas_info: &[u8; SIZE_CAS_INFO]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_byte(FunctionCode::ConClose as u8);
    PacketWriter::build_request(w.as_bytes(), cas_info)
}

/// Build an EndTran request (FC=1) for commit or rollback.
pub fn write_end_tran(
    tran_type: crate::constants::TransactionType,
    cas_info: &[u8; SIZE_CAS_INFO],
) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_byte(FunctionCode::EndTran as u8);
    w.add_byte(tran_type as u8);
    PacketWriter::build_request(w.as_bytes(), cas_info)
}

/// Build a GetDbVersion request (FC=15).
pub fn write_get_db_version(auto_commit: bool, cas_info: &[u8; SIZE_CAS_INFO]) -> Vec<u8> {
    let mut w = PacketWriter::new();
    w.write_byte(FunctionCode::GetDbVersion as u8);
    w.add_byte(if auto_commit { 1 } else { 0 });
    PacketWriter::build_request(w.as_bytes(), cas_info)
}

/// Parse a GetDbVersion response. Returns the version string.
pub fn parse_get_db_version(data: &[u8]) -> Result<(String, [u8; SIZE_CAS_INFO]), ProtocolError> {
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
    let version_len = reader.remaining();
    let version = if version_len > 0 {
        reader.parse_null_term_string(version_len)?
    } else {
        String::new()
    };
    Ok((version, cas_info))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_info_exchange() {
        let buf = write_client_info_exchange();
        assert_eq!(buf.len(), CLIENT_INFO_EXCHANGE_SIZE);
        assert_eq!(&buf[..5], b"CUBRK");
        assert_eq!(buf[5], CLIENT_TYPE_JDBC);
        assert_eq!(buf[6], CAS_VERSION);
        assert_eq!(&buf[7..], &[0, 0, 0]);
    }

    #[test]
    fn test_parse_client_info_exchange_positive() {
        let port = 33001i32;
        let data = port.to_be_bytes();
        assert_eq!(parse_client_info_exchange(&data), 33001);
    }

    #[test]
    fn test_parse_client_info_exchange_zero() {
        let data = 0i32.to_be_bytes();
        assert_eq!(parse_client_info_exchange(&data), 0);
    }

    #[test]
    fn test_parse_client_info_exchange_negative() {
        let data = (-1i32).to_be_bytes();
        assert_eq!(parse_client_info_exchange(&data), -1);
    }

    #[test]
    fn test_open_database_size() {
        let buf = write_open_database("testdb", "dba", "secret");
        assert_eq!(buf.len(), DB_OPEN_PAYLOAD_SIZE);
    }

    #[test]
    fn test_open_database_fields() {
        let buf = write_open_database("mydb", "user1", "pass1");
        // Database name at offset 0..32
        assert_eq!(&buf[..4], b"mydb");
        assert_eq!(buf[4], 0); // zero-padded

        // User at offset 32..64
        assert_eq!(&buf[32..37], b"user1");
        assert_eq!(buf[37], 0);

        // Password at offset 64..96
        assert_eq!(&buf[64..69], b"pass1");
        assert_eq!(buf[69], 0);

        // Filler and reserved should be all zeros
        assert!(buf[96..].iter().all(|&b| b == 0));
    }

    #[test]
    fn test_parse_open_database_success() {
        let mut buf = Vec::new();
        // CAS_INFO
        buf.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // response code (positive = success)
        buf.extend_from_slice(&1i32.to_be_bytes());
        // broker info (16 bytes) — put proto version in byte[4]
        let mut broker_info = [0u8; SIZE_BROKER_INFO];
        broker_info[4] = 7; // proto version 7
        buf.extend_from_slice(&broker_info);
        // session ID
        buf.extend_from_slice(&42i32.to_be_bytes());

        let result = parse_open_database(&buf).unwrap();
        assert_eq!(result.cas_info, [0x01, 0x02, 0x03, 0x04]);
        assert_eq!(result.proto_version, 7);
        assert_eq!(result.session_id, 42);
    }

    #[test]
    fn test_parse_open_database_error() {
        let mut buf = Vec::new();
        // CAS_INFO
        buf.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // response code (negative = error)
        buf.extend_from_slice(&(-1i32).to_be_bytes());
        // error code
        buf.extend_from_slice(&(-100i32).to_be_bytes());
        // error message
        buf.extend_from_slice(b"auth failed\0");

        let err = parse_open_database(&buf).unwrap_err();
        match err {
            ProtocolError::ServerError { code, message } => {
                assert_eq!(code, -100);
                assert_eq!(message, "auth failed");
            }
            other => panic!("expected ServerError, got {other:?}"),
        }
    }

    #[test]
    fn test_write_con_close() {
        let cas_info = [0x01, 0x02, 0x03, 0x04];
        let buf = write_con_close(&cas_info);
        // Should be: header (8) + payload (1 byte function code)
        assert_eq!(buf.len(), 9);
        assert_eq!(buf[8], FunctionCode::ConClose as u8);
    }

    #[test]
    fn test_write_end_tran_commit() {
        let cas_info = [0; SIZE_CAS_INFO];
        let buf = write_end_tran(TransactionType::Commit, &cas_info);
        // header (8) + FC byte (1) + add_byte (5) = 14
        assert_eq!(buf.len(), 14);
        assert_eq!(buf[8], FunctionCode::EndTran as u8);
    }

    #[test]
    fn test_write_get_db_version() {
        let cas_info = [0; SIZE_CAS_INFO];
        let buf = write_get_db_version(true, &cas_info);
        assert_eq!(buf[8], FunctionCode::GetDbVersion as u8);
    }

    #[test]
    fn test_parse_get_db_version_success() {
        let mut buf = Vec::new();
        // CAS_INFO
        buf.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // response code (positive)
        buf.extend_from_slice(&1i32.to_be_bytes());
        // version string
        buf.extend_from_slice(b"11.2.0\0");

        let (version, cas_info) = parse_get_db_version(&buf).unwrap();
        assert_eq!(version, "11.2.0");
        assert_eq!(cas_info, [0x01, 0x02, 0x03, 0x04]);
    }
}
