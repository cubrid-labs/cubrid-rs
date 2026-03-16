//! Connection pool for CUBRID clients.
//!
//! Provides connection pooling for both synchronous (`cubrid-client`) and
//! async (`cubrid-tokio`) CUBRID clients.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use thiserror::Error;

/// Pool errors.
#[derive(Debug, Error)]
pub enum Error {
    /// Sync client error.
    #[error("client error: {0}")]
    Client(#[from] cubrid_client::Error),

    /// Async client error.
    #[error("async client error: {0}")]
    AsyncClient(#[from] cubrid_tokio::Error),

    /// Pool exhausted - no available connections.
    #[error("pool exhausted: max {max} connections")]
    PoolExhausted {
        /// Maximum pool size.
        max: usize,
    },

    /// Pool is closed.
    #[error("pool closed")]
    PoolClosed,
}

/// Configuration for the connection pool.
pub struct PoolConfig {
    /// DSN string for CUBRID connection.
    pub dsn: String,
    /// Maximum number of connections in the pool.
    pub max_size: usize,
    /// Minimum number of idle connections to maintain.
    pub min_idle: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            dsn: String::new(),
            max_size: 10,
            min_idle: 1,
        }
    }
}
