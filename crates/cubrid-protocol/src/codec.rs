//! Packet framing — reading and writing length-prefixed, big-endian packets.
//!
//! The CAS protocol uses big-endian byte order for all multi-byte integers.
//!
//! ## Framing format
//!
//! After the initial handshake, all requests/responses are framed:
//! ```text
//! [DATA_LENGTH: 4 bytes BE u32] [CAS_INFO: 4 bytes] [payload: DATA_LENGTH bytes]
//! ```
//!
//! ## Writer conventions
//!
//! There are two families of write methods:
//! - `write_*` — raw writes (no length prefix)
//! - `add_*` — length-prefixed writes (4-byte length prefix + value)
//!
//! The distinction is critical: CAS protocol fields use a mix of both.

use crate::constants::*;
use crate::error::ProtocolError;

// ---------------------------------------------------------------------------
// PacketWriter
// ---------------------------------------------------------------------------

/// Builds binary packets for the CAS protocol.
///
/// All multi-byte numeric values are written in big-endian byte order.
pub struct PacketWriter {
    buf: Vec<u8>,
}

impl PacketWriter {
    /// Create a new packet writer with default capacity.
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(256),
        }
    }

    /// Create a new packet writer with the given capacity hint.
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
        }
    }

    /// Get the current length of the buffer.
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Get the buffer contents as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Consume the writer and return the buffer.
    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }

    /// Clear the buffer for reuse.
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    // -- Raw write methods (no length prefix) --

    /// Write a single byte.
    pub fn write_byte(&mut self, v: u8) {
        self.buf.push(v);
    }

    /// Write a 16-bit signed integer (big-endian).
    pub fn write_short(&mut self, v: i16) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    /// Write a 32-bit signed integer (big-endian).
    pub fn write_int(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    /// Write a 64-bit signed integer (big-endian).
    pub fn write_long(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    /// Write a 32-bit IEEE float (big-endian).
    pub fn write_float(&mut self, v: f32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    /// Write a 64-bit IEEE double (big-endian).
    pub fn write_double(&mut self, v: f64) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    /// Write raw bytes.
    pub fn write_raw_bytes(&mut self, v: &[u8]) {
        self.buf.extend_from_slice(v);
    }

    /// Write `count` zero bytes (filler/padding).
    pub fn write_filler(&mut self, count: usize) {
        self.buf.resize(self.buf.len() + count, 0);
    }

    /// Write a fixed-length string (padded with zeros or truncated).
    pub fn write_fixed_string(&mut self, s: &str, length: usize) {
        let bytes = s.as_bytes();
        if bytes.len() >= length {
            self.buf.extend_from_slice(&bytes[..length]);
        } else {
            self.buf.extend_from_slice(bytes);
            self.write_filler(length - bytes.len());
        }
    }

    /// Write a null-terminated string with a 4-byte length prefix.
    ///
    /// Wire format: `[i32(len(s) + 1)] [s bytes] [0x00]`
    pub fn write_null_term_string(&mut self, s: &str) {
        let bytes = s.as_bytes();
        self.write_int((bytes.len() + 1) as i32);
        self.buf.extend_from_slice(bytes);
        self.buf.push(0x00);
    }

    // -- Length-prefixed "add" methods --
    // These write `[i32(SIZE_X)] [value]` — a 4-byte length prefix followed by the value.

    /// Add a byte with length prefix: `[i32(1)] [u8]`.
    pub fn add_byte(&mut self, v: u8) {
        self.write_int(SIZE_BYTE as i32);
        self.write_byte(v);
    }

    /// Add a short with length prefix: `[i32(2)] [i16]`.
    pub fn add_short(&mut self, v: i16) {
        self.write_int(SIZE_SHORT as i32);
        self.write_short(v);
    }

    /// Add an int with length prefix: `[i32(4)] [i32]`.
    pub fn add_int(&mut self, v: i32) {
        self.write_int(SIZE_INT as i32);
        self.write_int(v);
    }

    /// Add a long with length prefix: `[i32(8)] [i64]`.
    pub fn add_long(&mut self, v: i64) {
        self.write_int(SIZE_LONG as i32);
        self.write_long(v);
    }

    /// Add a float with length prefix: `[i32(4)] [f32]`.
    pub fn add_float(&mut self, v: f32) {
        self.write_int(SIZE_FLOAT as i32);
        self.write_float(v);
    }

    /// Add a double with length prefix: `[i32(8)] [f64]`.
    pub fn add_double(&mut self, v: f64) {
        self.write_int(SIZE_DOUBLE as i32);
        self.write_double(v);
    }

    /// Add raw bytes with length prefix: `[i32(len)] [bytes]`.
    pub fn add_bytes(&mut self, v: &[u8]) {
        self.write_int(v.len() as i32);
        self.buf.extend_from_slice(v);
    }

    /// Add a NULL marker: `[i32(0)]`.
    pub fn add_null(&mut self) {
        self.write_int(0);
    }

    /// Add cache time fields: `[i32(SIZE_LONG)] [i32(0)] [i32(0)]`.
    pub fn add_cache_time(&mut self) {
        self.write_int(SIZE_LONG as i32);
        self.write_int(0); // sec
        self.write_int(0); // usec
    }

    // -- Framing --

    /// Build a framed protocol header: `[DATA_LENGTH: 4 BE] [CAS_INFO: 4]`.
    pub fn build_header(data_length: usize, cas_info: &[u8; SIZE_CAS_INFO]) -> [u8; 8] {
        let mut header = [0u8; 8];
        header[..4].copy_from_slice(&(data_length as u32).to_be_bytes());
        header[4..8].copy_from_slice(cas_info);
        header
    }

    /// Build a complete framed request: header + payload.
    pub fn build_request(payload: &[u8], cas_info: &[u8; SIZE_CAS_INFO]) -> Vec<u8> {
        let header = Self::build_header(payload.len(), cas_info);
        let mut request = Vec::with_capacity(8 + payload.len());
        request.extend_from_slice(&header);
        request.extend_from_slice(payload);
        request
    }
}

impl Default for PacketWriter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PacketReader
// ---------------------------------------------------------------------------

/// Reads binary packets from the CAS protocol.
///
/// All multi-byte numeric values are read in big-endian byte order.
pub struct PacketReader<'a> {
    data: &'a [u8],
    pos: usize,
}

type DateTimeParts = (i16, i16, i16, i16, i16, i16, i16);

impl<'a> PacketReader<'a> {
    /// Create a new packet reader over the given data.
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Get the current read position.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// Get the number of remaining unread bytes.
    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// Check if there are enough bytes remaining.
    fn ensure(&self, count: usize) -> Result<(), ProtocolError> {
        if self.remaining() < count {
            Err(ProtocolError::UnexpectedEof {
                expected: count,
                available: self.remaining(),
            })
        } else {
            Ok(())
        }
    }

    /// Read a single byte.
    pub fn parse_byte(&mut self) -> Result<u8, ProtocolError> {
        self.ensure(1)?;
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    /// Read an unsigned byte.
    pub fn parse_u_byte(&mut self) -> Result<u8, ProtocolError> {
        self.parse_byte()
    }

    /// Read a 16-bit signed integer (big-endian).
    pub fn parse_short(&mut self) -> Result<i16, ProtocolError> {
        self.ensure(2)?;
        let v = i16::from_be_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    /// Read a 32-bit signed integer (big-endian).
    pub fn parse_int(&mut self) -> Result<i32, ProtocolError> {
        self.ensure(4)?;
        let v = i32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    /// Read a 64-bit signed integer (big-endian).
    pub fn parse_long(&mut self) -> Result<i64, ProtocolError> {
        self.ensure(8)?;
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.data[self.pos..self.pos + 8]);
        self.pos += 8;
        Ok(i64::from_be_bytes(bytes))
    }

    /// Read a 32-bit IEEE float (big-endian).
    pub fn parse_float(&mut self) -> Result<f32, ProtocolError> {
        self.ensure(4)?;
        let v = f32::from_be_bytes([
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    /// Read a 64-bit IEEE double (big-endian).
    pub fn parse_double(&mut self) -> Result<f64, ProtocolError> {
        self.ensure(8)?;
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.data[self.pos..self.pos + 8]);
        self.pos += 8;
        Ok(f64::from_be_bytes(bytes))
    }

    /// Read `count` raw bytes.
    pub fn parse_raw_bytes(&mut self, count: usize) -> Result<Vec<u8>, ProtocolError> {
        self.ensure(count)?;
        let v = self.data[self.pos..self.pos + count].to_vec();
        self.pos += count;
        Ok(v)
    }

    /// Read a null-terminated string of the given length (including the null terminator).
    ///
    /// Strips trailing null bytes.
    pub fn parse_null_term_string(&mut self, length: usize) -> Result<String, ProtocolError> {
        if length == 0 {
            return Ok(String::new());
        }
        self.ensure(length)?;
        let raw = &self.data[self.pos..self.pos + length];
        self.pos += length;
        // Strip trailing null bytes
        let end = raw.iter().rposition(|&b| b != 0).map_or(0, |p| p + 1);
        String::from_utf8(raw[..end].to_vec())
            .map_err(|e| ProtocolError::InvalidPacket(format!("invalid UTF-8: {e}")))
    }

    /// Skip `count` bytes.
    pub fn skip(&mut self, count: usize) -> Result<(), ProtocolError> {
        self.ensure(count)?;
        self.pos += count;
        Ok(())
    }

    /// Read CAS_INFO (4 bytes) and return as a fixed array.
    pub fn parse_cas_info(&mut self) -> Result<[u8; SIZE_CAS_INFO], ProtocolError> {
        self.ensure(SIZE_CAS_INFO)?;
        let mut info = [0u8; SIZE_CAS_INFO];
        info.copy_from_slice(&self.data[self.pos..self.pos + SIZE_CAS_INFO]);
        self.pos += SIZE_CAS_INFO;
        Ok(info)
    }

    /// Read an error response: `[i32 code] [null-term-string message]`.
    pub fn read_error(&mut self, response_length: usize) -> Result<(i32, String), ProtocolError> {
        let code = self.parse_int()?;
        let msg_len = response_length.saturating_sub(SIZE_INT);
        let message = if msg_len > 0 {
            self.parse_null_term_string(msg_len)?
        } else {
            String::new()
        };
        Ok((code, message))
    }

    // -- Date/Time parsing --

    /// Parse a DATE value: 3 × i16 (year, month, day).
    pub fn parse_date(&mut self) -> Result<(i16, i16, i16), ProtocolError> {
        let year = self.parse_short()?;
        let month = self.parse_short()?;
        let day = self.parse_short()?;
        Ok((year, month, day))
    }

    /// Parse a TIME value: 3 × i16 (hour, minute, second).
    pub fn parse_time(&mut self) -> Result<(i16, i16, i16), ProtocolError> {
        let hour = self.parse_short()?;
        let minute = self.parse_short()?;
        let second = self.parse_short()?;
        Ok((hour, minute, second))
    }

    /// Parse a TIMESTAMP value: 6 × i16 (year, month, day, hour, minute, second).
    pub fn parse_timestamp(&mut self) -> Result<(i16, i16, i16, i16, i16, i16), ProtocolError> {
        let year = self.parse_short()?;
        let month = self.parse_short()?;
        let day = self.parse_short()?;
        let hour = self.parse_short()?;
        let minute = self.parse_short()?;
        let second = self.parse_short()?;
        Ok((year, month, day, hour, minute, second))
    }

    /// Parse a DATETIME value: 7 × i16 (year, month, day, hour, minute, second, ms).
    pub fn parse_datetime(&mut self) -> Result<DateTimeParts, ProtocolError> {
        let year = self.parse_short()?;
        let month = self.parse_short()?;
        let day = self.parse_short()?;
        let hour = self.parse_short()?;
        let minute = self.parse_short()?;
        let second = self.parse_short()?;
        let ms = self.parse_short()?;
        Ok((year, month, day, hour, minute, second, ms))
    }
}

/// Parse a framed protocol header from 8 bytes.
///
/// Returns `(data_length, cas_info)`.
pub fn parse_protocol_header(data: &[u8; 8]) -> (usize, [u8; SIZE_CAS_INFO]) {
    let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut cas_info = [0u8; SIZE_CAS_INFO];
    cas_info.copy_from_slice(&data[4..8]);
    (len, cas_info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_writer_write_byte() {
        let mut w = PacketWriter::new();
        w.write_byte(0x42);
        assert_eq!(w.as_bytes(), &[0x42]);
    }

    #[test]
    fn test_writer_write_short() {
        let mut w = PacketWriter::new();
        w.write_short(0x0102);
        assert_eq!(w.as_bytes(), &[0x01, 0x02]);
    }

    #[test]
    fn test_writer_write_int() {
        let mut w = PacketWriter::new();
        w.write_int(0x01020304);
        assert_eq!(w.as_bytes(), &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_writer_write_long() {
        let mut w = PacketWriter::new();
        w.write_long(0x0102030405060708);
        assert_eq!(
            w.as_bytes(),
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]
        );
    }

    #[test]
    fn test_writer_write_float() {
        let mut w = PacketWriter::new();
        w.write_float(1.0);
        assert_eq!(w.as_bytes(), &1.0f32.to_be_bytes());
    }

    #[test]
    fn test_writer_write_double() {
        let mut w = PacketWriter::new();
        w.write_double(1.0);
        assert_eq!(w.as_bytes(), &1.0f64.to_be_bytes());
    }

    #[test]
    fn test_writer_write_filler() {
        let mut w = PacketWriter::new();
        w.write_filler(5);
        assert_eq!(w.as_bytes(), &[0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_writer_write_fixed_string() {
        let mut w = PacketWriter::new();
        w.write_fixed_string("abc", 5);
        assert_eq!(w.as_bytes(), &[b'a', b'b', b'c', 0, 0]);
    }

    #[test]
    fn test_writer_write_fixed_string_truncate() {
        let mut w = PacketWriter::new();
        w.write_fixed_string("abcdef", 3);
        assert_eq!(w.as_bytes(), b"abc");
    }

    #[test]
    fn test_writer_write_null_term_string() {
        let mut w = PacketWriter::new();
        w.write_null_term_string("hi");
        // Expected: i32(3) + "hi" + 0x00
        let mut expected = Vec::new();
        expected.extend_from_slice(&3i32.to_be_bytes());
        expected.extend_from_slice(b"hi");
        expected.push(0x00);
        assert_eq!(w.as_bytes(), &expected);
    }

    #[test]
    fn test_writer_add_byte() {
        let mut w = PacketWriter::new();
        w.add_byte(0x42);
        // Expected: i32(1) + 0x42
        let mut expected = Vec::new();
        expected.extend_from_slice(&1i32.to_be_bytes());
        expected.push(0x42);
        assert_eq!(w.as_bytes(), &expected);
    }

    #[test]
    fn test_writer_add_int() {
        let mut w = PacketWriter::new();
        w.add_int(42);
        // Expected: i32(4) + i32(42)
        let mut expected = Vec::new();
        expected.extend_from_slice(&4i32.to_be_bytes());
        expected.extend_from_slice(&42i32.to_be_bytes());
        assert_eq!(w.as_bytes(), &expected);
    }

    #[test]
    fn test_writer_add_bytes() {
        let mut w = PacketWriter::new();
        w.add_bytes(&[1, 2, 3]);
        let mut expected = Vec::new();
        expected.extend_from_slice(&3i32.to_be_bytes());
        expected.extend_from_slice(&[1, 2, 3]);
        assert_eq!(w.as_bytes(), &expected);
    }

    #[test]
    fn test_writer_add_null() {
        let mut w = PacketWriter::new();
        w.add_null();
        assert_eq!(w.as_bytes(), &0i32.to_be_bytes());
    }

    #[test]
    fn test_writer_add_cache_time() {
        let mut w = PacketWriter::new();
        w.add_cache_time();
        let mut expected = Vec::new();
        expected.extend_from_slice(&(SIZE_LONG as i32).to_be_bytes());
        expected.extend_from_slice(&0i32.to_be_bytes());
        expected.extend_from_slice(&0i32.to_be_bytes());
        assert_eq!(w.as_bytes(), &expected);
    }

    #[test]
    fn test_build_header() {
        let cas_info = [0x01, 0x02, 0x03, 0x04];
        let header = PacketWriter::build_header(100, &cas_info);
        let mut expected = [0u8; 8];
        expected[..4].copy_from_slice(&100u32.to_be_bytes());
        expected[4..8].copy_from_slice(&cas_info);
        assert_eq!(header, expected);
    }

    #[test]
    fn test_build_request() {
        let cas_info = [0xAA, 0xBB, 0xCC, 0xDD];
        let payload = vec![0x01, 0x02, 0x03];
        let request = PacketWriter::build_request(&payload, &cas_info);
        assert_eq!(request.len(), 8 + 3);
        assert_eq!(&request[..4], &3u32.to_be_bytes());
        assert_eq!(&request[4..8], &cas_info);
        assert_eq!(&request[8..], &[0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_reader_parse_byte() {
        let data = [0x42];
        let mut r = PacketReader::new(&data);
        assert_eq!(r.parse_byte().unwrap(), 0x42);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn test_reader_parse_short() {
        let data = 0x0102i16.to_be_bytes();
        let mut r = PacketReader::new(&data);
        assert_eq!(r.parse_short().unwrap(), 0x0102);
    }

    #[test]
    fn test_reader_parse_int() {
        let data = 42i32.to_be_bytes();
        let mut r = PacketReader::new(&data);
        assert_eq!(r.parse_int().unwrap(), 42);
    }

    #[test]
    fn test_reader_parse_long() {
        let data = 123456789i64.to_be_bytes();
        let mut r = PacketReader::new(&data);
        assert_eq!(r.parse_long().unwrap(), 123456789);
    }

    #[test]
    fn test_reader_parse_float() {
        let data = std::f32::consts::PI.to_be_bytes();
        let mut r = PacketReader::new(&data);
        let v = r.parse_float().unwrap();
        assert!((v - std::f32::consts::PI).abs() < 1e-5);
    }

    #[test]
    fn test_reader_parse_double() {
        let data = std::f64::consts::PI.to_be_bytes();
        let mut r = PacketReader::new(&data);
        let v = r.parse_double().unwrap();
        assert!((v - std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn test_reader_parse_null_term_string() {
        let data = [b'h', b'i', 0x00];
        let mut r = PacketReader::new(&data);
        assert_eq!(r.parse_null_term_string(3).unwrap(), "hi");
    }

    #[test]
    fn test_reader_parse_null_term_string_empty() {
        let mut r = PacketReader::new(&[]);
        assert_eq!(r.parse_null_term_string(0).unwrap(), "");
    }

    #[test]
    fn test_reader_parse_cas_info() {
        let data = [0x01, 0x02, 0x03, 0x04, 0xFF];
        let mut r = PacketReader::new(&data);
        let info = r.parse_cas_info().unwrap();
        assert_eq!(info, [0x01, 0x02, 0x03, 0x04]);
        assert_eq!(r.remaining(), 1);
    }

    #[test]
    fn test_reader_read_error() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(-1234i32).to_be_bytes());
        buf.extend_from_slice(b"test error\0");
        let mut r = PacketReader::new(&buf);
        let (code, msg) = r.read_error(buf.len()).unwrap();
        assert_eq!(code, -1234);
        assert_eq!(msg, "test error");
    }

    #[test]
    fn test_reader_parse_date() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&2024i16.to_be_bytes());
        buf.extend_from_slice(&3i16.to_be_bytes());
        buf.extend_from_slice(&15i16.to_be_bytes());
        let mut r = PacketReader::new(&buf);
        let (y, m, d) = r.parse_date().unwrap();
        assert_eq!((y, m, d), (2024, 3, 15));
    }

    #[test]
    fn test_reader_parse_datetime() {
        let mut buf = Vec::new();
        for v in [2024i16, 3, 15, 10, 30, 45, 123] {
            buf.extend_from_slice(&v.to_be_bytes());
        }
        let mut r = PacketReader::new(&buf);
        let dt = r.parse_datetime().unwrap();
        assert_eq!(dt, (2024, 3, 15, 10, 30, 45, 123));
    }

    #[test]
    fn test_reader_skip() {
        let data = [0x01, 0x02, 0x03, 0x04];
        let mut r = PacketReader::new(&data);
        r.skip(2).unwrap();
        assert_eq!(r.remaining(), 2);
        assert_eq!(r.parse_byte().unwrap(), 0x03);
    }

    #[test]
    fn test_reader_eof_error() {
        let data = [0x01];
        let mut r = PacketReader::new(&data);
        let err = r.parse_int().unwrap_err();
        match err {
            ProtocolError::UnexpectedEof {
                expected: 4,
                available: 1,
            } => {}
            other => panic!("expected UnexpectedEof, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_protocol_header() {
        let mut header = [0u8; 8];
        header[..4].copy_from_slice(&256u32.to_be_bytes());
        header[4..8].copy_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);
        let (len, cas_info) = parse_protocol_header(&header);
        assert_eq!(len, 256);
        assert_eq!(cas_info, [0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn test_roundtrip_writer_reader() {
        let mut w = PacketWriter::new();
        w.write_byte(0x42);
        w.write_short(1234);
        w.write_int(56789);
        w.write_long(1234567890);
        w.write_null_term_string("hello");

        let data = w.into_bytes();
        let mut r = PacketReader::new(&data);
        assert_eq!(r.parse_byte().unwrap(), 0x42);
        assert_eq!(r.parse_short().unwrap(), 1234);
        assert_eq!(r.parse_int().unwrap(), 56789);
        assert_eq!(r.parse_long().unwrap(), 1234567890);
        // null_term_string: read length prefix first
        let str_len = r.parse_int().unwrap() as usize;
        assert_eq!(r.parse_null_term_string(str_len).unwrap(), "hello");
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn test_writer_len_and_clear() {
        let mut w = PacketWriter::new();
        assert!(w.is_empty());
        w.write_byte(0x01);
        assert_eq!(w.len(), 1);
        assert!(!w.is_empty());
        w.clear();
        assert!(w.is_empty());
    }

    #[test]
    fn test_writer_add_short() {
        let mut w = PacketWriter::new();
        w.add_short(42);
        let mut expected = Vec::new();
        expected.extend_from_slice(&2i32.to_be_bytes());
        expected.extend_from_slice(&42i16.to_be_bytes());
        assert_eq!(w.as_bytes(), &expected);
    }

    #[test]
    fn test_writer_add_long() {
        let mut w = PacketWriter::new();
        w.add_long(123456789);
        let mut expected = Vec::new();
        expected.extend_from_slice(&8i32.to_be_bytes());
        expected.extend_from_slice(&123456789i64.to_be_bytes());
        assert_eq!(w.as_bytes(), &expected);
    }

    #[test]
    fn test_writer_add_float() {
        let mut w = PacketWriter::new();
        w.add_float(1.5);
        let mut expected = Vec::new();
        expected.extend_from_slice(&4i32.to_be_bytes());
        expected.extend_from_slice(&1.5f32.to_be_bytes());
        assert_eq!(w.as_bytes(), &expected);
    }

    #[test]
    fn test_writer_add_double() {
        let mut w = PacketWriter::new();
        w.add_double(2.5);
        let mut expected = Vec::new();
        expected.extend_from_slice(&8i32.to_be_bytes());
        expected.extend_from_slice(&2.5f64.to_be_bytes());
        assert_eq!(w.as_bytes(), &expected);
    }

    #[test]
    fn test_writer_default() {
        let w = PacketWriter::default();
        assert!(w.is_empty());
    }

    #[test]
    fn test_reader_position() {
        let data = [0x01, 0x02, 0x03, 0x04];
        let mut r = PacketReader::new(&data);
        assert_eq!(r.position(), 0);
        r.parse_byte().unwrap();
        assert_eq!(r.position(), 1);
        r.parse_short().unwrap();
        assert_eq!(r.position(), 3);
    }

    #[test]
    fn test_reader_parse_u_byte() {
        let data = [0xFF];
        let mut r = PacketReader::new(&data);
        assert_eq!(r.parse_u_byte().unwrap(), 0xFF);
    }

    #[test]
    fn test_reader_read_error_empty_message() {
        let buf = (-999i32).to_be_bytes();
        let mut r = PacketReader::new(&buf);
        let (code, msg) = r.read_error(4).unwrap();
        assert_eq!(code, -999);
        assert_eq!(msg, "");
    }
}
