//! Async CUBRID database client built on tokio.
//!
//! Provides a non-blocking, async TCP client for CUBRID using the CAS
//! wire protocol with tokio for I/O.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use cubrid_tokio::Client;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), cubrid_tokio::Error> {
//!     let mut client = Client::connect("cubrid://dba:@localhost:33000/demodb").await?;
//!     let rows = client.query("SELECT 1 + 1", &[]).await?;
//!     Ok(())
//! }
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use thiserror::Error;

/// Async client errors.
#[derive(Debug, Error)]
pub enum Error {
    /// Protocol-level error.
    #[error("protocol error: {0}")]
    Protocol(#[from] cubrid_protocol::ProtocolError),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid DSN format.
    #[error("invalid DSN: {0}")]
    InvalidDsn(String),

    /// Connection is closed.
    #[error("connection closed")]
    ConnectionClosed,
}

/// An async CUBRID database client using tokio.
pub struct Client {
    _placeholder: (),
}

impl Client {
    /// Connect to a CUBRID database asynchronously.
    ///
    /// # DSN Format
    ///
    /// ```text
    /// cubrid://[user[:password]]@host[:port]/database
    /// ```
    pub async fn connect(_dsn: &str) -> Result<Self, Error> {
        todo!("Async connection implementation pending")
    }

    /// Execute a query and return results.
    pub async fn query(&mut self, _sql: &str, _params: &[&str]) -> Result<Vec<()>, Error> {
        todo!("Query implementation pending")
    }
}
