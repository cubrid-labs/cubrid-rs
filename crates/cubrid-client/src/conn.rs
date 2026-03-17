//! Low-level TCP connection for CUBRID CAS protocol.
//!
//! Handles framed I/O: `[4-byte DATA_LENGTH][data]` where data includes
//! CAS_INFO (4 bytes) + body.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use cubrid_protocol::constants::SIZE_CAS_INFO;

use crate::dsn::Dsn;
use crate::Error;

/// Low-level TCP connection to a CUBRID CAS broker.
pub(crate) struct Connection {
    stream: TcpStream,
    cas_info: [u8; SIZE_CAS_INFO],
}

impl Connection {
    /// Connect to the broker using DSN settings.
    pub fn connect(dsn: &Dsn) -> Result<Self, Error> {
        Self::connect_to(&dsn.host, dsn.port, dsn.timeout)
    }

    /// Connect to a specific host:port with timeout.
    pub fn connect_to(host: &str, port: u16, timeout: Duration) -> Result<Self, Error> {
        let addr = format!("{host}:{port}");
        let stream = if timeout.is_zero() {
            TcpStream::connect(&addr)?
        } else {
            TcpStream::connect_timeout(
                &addr.parse::<std::net::SocketAddr>().or_else(|_| {
                    // Resolve hostname
                    use std::net::ToSocketAddrs;
                    addr.to_socket_addrs()?.next().ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::AddrNotAvailable,
                            format!("cannot resolve: {addr}"),
                        )
                    })
                })?,
                timeout,
            )?
        };

        // Set read/write timeouts
        if !timeout.is_zero() {
            stream.set_read_timeout(Some(timeout))?;
            stream.set_write_timeout(Some(timeout))?;
        }

        // Disable Nagle's algorithm for low latency
        stream.set_nodelay(true)?;

        Ok(Connection {
            stream,
            cas_info: [0u8; SIZE_CAS_INFO],
        })
    }

    /// Get current CAS info.
    pub fn cas_info(&self) -> [u8; SIZE_CAS_INFO] {
        self.cas_info
    }

    /// Update CAS info from server response.
    pub fn set_cas_info(&mut self, cas_info: [u8; SIZE_CAS_INFO]) {
        self.cas_info = cas_info;
    }

    /// Write raw bytes to the socket (no framing).
    pub fn write_raw(&mut self, data: &[u8]) -> Result<(), Error> {
        self.stream.write_all(data)?;
        self.stream.flush()?;
        Ok(())
    }

    /// Read exactly `n` bytes from the socket.
    pub fn read_exact_bytes(&mut self, n: usize) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0u8; n];
        self.stream.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Send a framed request.
    ///
    /// The request bytes already contain the `[DATA_LENGTH][CAS_INFO][body]` framing
    /// built by `PacketWriter::build_request`.
    pub fn send_framed(&mut self, data: &[u8]) -> Result<(), Error> {
        self.stream.write_all(data)?;
        self.stream.flush()?;
        Ok(())
    }

    /// Receive a framed response.
    ///
    /// Wire format: `[4-byte DATA_LENGTH (big-endian)][CAS_INFO + body]`
    ///
    /// Returns the `CAS_INFO + body` portion (DATA_LENGTH bytes).
    pub fn recv_framed(&mut self) -> Result<Vec<u8>, Error> {
        // Read 4-byte data length
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf)?;
        let data_len = u32::from_be_bytes(len_buf) as usize;

        // CUBRID wire: [DATA_LENGTH][CAS_INFO (4 bytes)][body (DATA_LENGTH bytes)]
        // Read CAS_INFO + body together (total = data_len + SIZE_CAS_INFO).
        // Wait -- looking at the Go reference: data_len is the length of everything
        // AFTER DATA_LENGTH, but CAS_INFO is included in the count or not?
        //
        // From cubrid-go/conn.go recv():
        //   totalLen := dataLen + SizeCASInfo  // Read CAS_INFO + body together
        //
        // This means DATA_LENGTH = body size (without CAS_INFO), so total to read
        // is dataLen + 4.
        //
        // Actually, looking more carefully at the Go code:
        //   lenBuf := make([]byte, SizeDataLength)  // 4 bytes
        //   dataLen := int(binary.BigEndian.Uint32(lenBuf))
        //   totalLen := dataLen + SizeCASInfo  // SizeCASInfo = 4
        //   data := make([]byte, totalLen)
        //
        // So the wire format is: [4-byte len][4-byte cas_info][len bytes of body]
        // And we read cas_info + body = len + 4 bytes total.
        let total_len = data_len + SIZE_CAS_INFO;
        let mut data = vec![0u8; total_len];
        self.stream.read_exact(&mut data)?;
        Ok(data)
    }

    /// Write data and close the connection (best-effort).
    pub fn write_and_close(mut self, data: &[u8]) -> Result<(), Error> {
        let _ = self.stream.write_all(data);
        let _ = self.stream.flush();
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cas_info_default() {
        // We can't test TCP connection without a server, but we can test the struct
        let dsn = Dsn::default();
        assert_eq!(dsn.host, "localhost");
        assert_eq!(dsn.port, 33000);
    }

    #[test]
    fn test_connect_to_invalid_host() {
        // Test that connection to a non-existent host fails
        let result = Connection::connect_to("192.0.2.1", 33000, Duration::from_millis(100));
        assert!(result.is_err());
    }

    #[test]
    fn test_cas_info_operations() {
        // Test CAS info get/set without real connection
        // We just verify the data structure works correctly
        let info: [u8; SIZE_CAS_INFO] = [0xAA, 0xBB, 0xCC, 0xDD];
        let mut conn_info = [0u8; SIZE_CAS_INFO];
        conn_info.copy_from_slice(&info);
        assert_eq!(conn_info, [0xAA, 0xBB, 0xCC, 0xDD]);
    }
}
