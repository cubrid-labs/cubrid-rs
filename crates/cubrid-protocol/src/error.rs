//! Error types for protocol operations.

use thiserror::Error;

/// Errors that can occur during protocol operations.
#[derive(Debug, Error)]
pub enum ProtocolError {
    /// I/O error during read/write.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid packet data received.
    #[error("invalid packet: {0}")]
    InvalidPacket(String),

    /// Server returned an error response.
    #[error("server error {code}: {message}")]
    ServerError {
        /// CUBRID error code.
        code: i32,
        /// Error message from server.
        message: String,
    },

    /// Authentication failed.
    #[error("authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Unsupported data type encountered.
    #[error("unsupported data type: {0}")]
    UnsupportedType(u8),

    /// Invalid DSN format.
    #[error("invalid DSN: {0}")]
    InvalidDsn(String),

    /// Connection was closed.
    #[error("connection closed")]
    ConnectionClosed,

    /// Unexpected end of data while parsing.
    #[error("unexpected end of data: expected {expected} bytes, got {available}")]
    UnexpectedEof {
        /// Number of bytes expected.
        expected: usize,
        /// Number of bytes available.
        available: usize,
    },
}
