//! Synchronous CUBRID database client.
//!
//! Provides a blocking TCP client for CUBRID using the CAS wire protocol.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use cubrid_client::Client;
//!
//! let mut client = Client::connect("cubrid://dba:@localhost:33000/demodb")?;
//! let rows = client.query("SELECT * FROM athlete WHERE nation_code = ?", &["KOR"])?;
//! for row in rows {
//!     println!("{:?}", row);
//! }
//! # Ok::<(), cubrid_client::Error>(())
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use thiserror::Error;

/// Client errors.
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

/// A synchronous CUBRID database client.
///
/// Connects to a CUBRID broker via TCP and communicates using the CAS
/// wire protocol. All operations are blocking.
pub struct Client {
    _placeholder: (),
}

impl Client {
    /// Connect to a CUBRID database.
    ///
    /// # DSN Format
    ///
    /// ```text
    /// cubrid://[user[:password]]@host[:port]/database
    /// ```
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use cubrid_client::Client;
    ///
    /// let mut client = Client::connect("cubrid://dba:@localhost:33000/demodb")?;
    /// # Ok::<(), cubrid_client::Error>(())
    /// ```
    pub fn connect(_dsn: &str) -> Result<Self, Error> {
        todo!("Connection implementation pending - see cubrid-protocol for wire protocol")
    }
}
