//! Async TCP connection for CUBRID CAS protocol.
//!
//! Uses `tokio::net::TcpStream` with `AsyncRead`/`AsyncWrite` for non-blocking I/O.
//! Handles framed protocol: `[4-byte DATA_LENGTH][CAS_INFO + body]`.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use cubrid_protocol::constants::SIZE_CAS_INFO;

use crate::Error;

/// Async TCP connection to a CUBRID CAS broker.
pub(crate) struct AsyncConnection {
    stream: TcpStream,
    cas_info: [u8; SIZE_CAS_INFO],
}

impl AsyncConnection {
    /// Connect to host:port with optional timeout.
    pub async fn connect(host: &str, port: u16, timeout: Duration) -> Result<Self, Error> {
        let addr = format!("{host}:{port}");
        let stream = if timeout.is_zero() {
            TcpStream::connect(&addr).await?
        } else {
            tokio::time::timeout(timeout, TcpStream::connect(&addr))
                .await
                .map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::TimedOut, "connection timed out")
                })??
        };

        // Disable Nagle's algorithm for low latency
        stream.set_nodelay(true)?;

        Ok(AsyncConnection {
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
    pub async fn write_raw(&mut self, data: &[u8]) -> Result<(), Error> {
        self.stream.write_all(data).await?;
        self.stream.flush().await?;
        Ok(())
    }

    /// Read exactly `n` bytes from the socket.
    pub async fn read_exact_bytes(&mut self, n: usize) -> Result<Vec<u8>, Error> {
        let mut buf = vec![0u8; n];
        self.stream.read_exact(&mut buf).await?;
        Ok(buf)
    }

    /// Send a framed request.
    ///
    /// The request bytes already contain the `[DATA_LENGTH][CAS_INFO][body]` framing
    /// built by `PacketWriter::build_request`.
    pub async fn send_framed(&mut self, data: &[u8]) -> Result<(), Error> {
        self.stream.write_all(data).await?;
        self.stream.flush().await?;
        Ok(())
    }

    /// Receive a framed response.
    ///
    /// Wire format: `[4-byte DATA_LENGTH (big-endian)][CAS_INFO + body]`
    ///
    /// Returns the `CAS_INFO + body` portion.
    /// DATA_LENGTH = body size (without CAS_INFO), so total to read = dataLen + 4.
    pub async fn recv_framed(&mut self) -> Result<Vec<u8>, Error> {
        // Read 4-byte data length
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let data_len = u32::from_be_bytes(len_buf) as usize;

        // Read CAS_INFO + body = data_len + SIZE_CAS_INFO bytes
        let total_len = data_len + SIZE_CAS_INFO;
        let mut data = vec![0u8; total_len];
        self.stream.read_exact(&mut data).await?;
        Ok(data)
    }

    /// Write data and close the connection (best-effort).
    pub async fn write_and_close(mut self, data: &[u8]) -> Result<(), Error> {
        let _ = self.stream.write_all(data).await;
        let _ = self.stream.flush().await;
        let _ = self.stream.shutdown().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connect_invalid_host() {
        let result =
            AsyncConnection::connect("192.0.2.1", 33000, Duration::from_millis(100)).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_cas_info_default() {
        let info = [0u8; SIZE_CAS_INFO];
        assert_eq!(info, [0, 0, 0, 0]);
    }

    #[test]
    fn test_cas_info_operations() {
        let info: [u8; SIZE_CAS_INFO] = [0xAA, 0xBB, 0xCC, 0xDD];
        let mut conn_info = [0u8; SIZE_CAS_INFO];
        conn_info.copy_from_slice(&info);
        assert_eq!(conn_info, [0xAA, 0xBB, 0xCC, 0xDD]);
    }
}
