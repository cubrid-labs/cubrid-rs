//! Connection pool for CUBRID clients.
//!
//! Provides connection pooling for both synchronous (`cubrid-client`) and
//! async (`cubrid-tokio`) CUBRID clients.
//!
//! # Sync Pool Example
//!
//! ```rust,no_run
//! use cubrid_pool::{SyncPool, PoolConfig};
//!
//! let config = PoolConfig::new("cubrid://dba:@localhost:33000/demodb")
//!     .max_size(10)
//!     .min_idle(2);
//!
//! let pool = SyncPool::new(config)?;
//!
//! // Get a connection from the pool
//! let mut conn = pool.get()?;
//! let rows = conn.query("SELECT 1 + 1", &[])?;
//! drop(conn); // Returns connection to pool
//!
//! pool.close();
//! # Ok::<(), cubrid_pool::Error>(())
//! ```
//!
//! # Async Pool Example
//!
//! ```rust,no_run
//! use cubrid_pool::{AsyncPool, PoolConfig};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), cubrid_pool::Error> {
//! let config = PoolConfig::new("cubrid://dba:@localhost:33000/demodb")
//!     .max_size(10)
//!     .min_idle(2);
//!
//! let pool = AsyncPool::new(config).await?;
//!
//! // Get a connection from the pool
//! let mut conn = pool.get().await?;
//! let rows = conn.query("SELECT 1 + 1", &[]).await?;
//! drop(conn); // Returns connection to pool
//!
//! pool.close().await;
//! # Ok(())
//! # }
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

use thiserror::Error;
use tokio::sync::Mutex as AsyncMutex;

/// Pool errors.
#[derive(Debug, Error)]
pub enum Error {
    /// Sync client error.
    #[error("client error: {0}")]
    Client(#[from] cubrid_client::Error),

    /// Async client error.
    #[error("async client error: {0}")]
    AsyncClient(#[from] cubrid_tokio::Error),

    /// Pool exhausted — no available connections and at max capacity.
    #[error("pool exhausted: max {max} connections")]
    PoolExhausted {
        /// Maximum pool size.
        max: usize,
    },

    /// Pool is closed.
    #[error("pool closed")]
    PoolClosed,

    /// Invalid configuration.
    #[error("invalid config: {0}")]
    InvalidConfig(String),
}

/// Configuration for the connection pool.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// DSN string for CUBRID connection.
    pub dsn: String,
    /// Maximum number of connections in the pool.
    pub max_size: usize,
    /// Minimum number of idle connections to maintain.
    pub min_idle: usize,
}

impl PoolConfig {
    /// Create a new pool config with the given DSN.
    pub fn new(dsn: &str) -> Self {
        Self {
            dsn: dsn.to_string(),
            max_size: 10,
            min_idle: 1,
        }
    }

    /// Set maximum pool size.
    pub fn max_size(mut self, max_size: usize) -> Self {
        self.max_size = max_size;
        self
    }

    /// Set minimum idle connections.
    pub fn min_idle(mut self, min_idle: usize) -> Self {
        self.min_idle = min_idle;
        self
    }

    fn validate(&self) -> Result<(), Error> {
        if self.dsn.is_empty() {
            return Err(Error::InvalidConfig("DSN cannot be empty".to_string()));
        }
        if self.max_size == 0 {
            return Err(Error::InvalidConfig(
                "max_size must be greater than 0".to_string(),
            ));
        }
        if self.min_idle > self.max_size {
            return Err(Error::InvalidConfig(
                "min_idle cannot exceed max_size".to_string(),
            ));
        }
        Ok(())
    }
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

// ─── Sync Pool ───────────────────────────────────────────────────────────────

struct SyncPoolInner {
    config: PoolConfig,
    idle: VecDeque<cubrid_client::Client>,
    active_count: usize,
    closed: bool,
}

/// A synchronous connection pool for `cubrid_client::Client`.
///
/// Thread-safe. Connections are returned to the pool when dropped.
pub struct SyncPool {
    inner: Arc<Mutex<SyncPoolInner>>,
}

impl SyncPool {
    /// Create a new sync connection pool.
    ///
    /// Pre-creates `min_idle` connections on construction.
    pub fn new(config: PoolConfig) -> Result<Self, Error> {
        config.validate()?;

        let mut idle = VecDeque::with_capacity(config.max_size);
        for _ in 0..config.min_idle {
            let client = cubrid_client::Client::connect(&config.dsn)?;
            idle.push_back(client);
        }

        Ok(SyncPool {
            inner: Arc::new(Mutex::new(SyncPoolInner {
                config,
                idle,
                active_count: 0,
                closed: false,
            })),
        })
    }

    /// Get a connection from the pool.
    ///
    /// If an idle connection is available, it is returned.
    /// If the pool hasn't reached `max_size`, a new connection is created.
    /// Otherwise, returns `Error::PoolExhausted`.
    pub fn get(&self) -> Result<SyncPooledConnection, Error> {
        let mut inner = self.inner.lock().unwrap();
        if inner.closed {
            return Err(Error::PoolClosed);
        }

        // Try to get an idle connection
        if let Some(client) = inner.idle.pop_front() {
            inner.active_count += 1;
            return Ok(SyncPooledConnection {
                client: Some(client),
                pool: Arc::clone(&self.inner),
            });
        }

        // Create a new connection if under max
        let total = inner.active_count + inner.idle.len();
        if total < inner.config.max_size {
            let dsn = inner.config.dsn.clone();
            inner.active_count += 1;
            // Drop lock before connecting
            drop(inner);
            match cubrid_client::Client::connect(&dsn) {
                Ok(client) => Ok(SyncPooledConnection {
                    client: Some(client),
                    pool: Arc::clone(&self.inner),
                }),
                Err(e) => {
                    // Decrement active count on failure
                    let mut inner = self.inner.lock().unwrap();
                    inner.active_count -= 1;
                    Err(e.into())
                }
            }
        } else {
            Err(Error::PoolExhausted {
                max: inner.config.max_size,
            })
        }
    }

    /// Get pool statistics.
    pub fn status(&self) -> PoolStatus {
        let inner = self.inner.lock().unwrap();
        PoolStatus {
            max_size: inner.config.max_size,
            idle_count: inner.idle.len(),
            active_count: inner.active_count,
            closed: inner.closed,
        }
    }

    /// Close the pool and all idle connections.
    pub fn close(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.closed = true;
        for mut client in inner.idle.drain(..) {
            let _ = client.close();
        }
    }
}

/// A connection from the sync pool.
///
/// Automatically returns to the pool when dropped.
/// Derefs to `cubrid_client::Client` for transparent usage.
pub struct SyncPooledConnection {
    client: Option<cubrid_client::Client>,
    pool: Arc<Mutex<SyncPoolInner>>,
}

impl Deref for SyncPooledConnection {
    type Target = cubrid_client::Client;

    fn deref(&self) -> &Self::Target {
        self.client.as_ref().unwrap()
    }
}

impl DerefMut for SyncPooledConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.client.as_mut().unwrap()
    }
}

impl Drop for SyncPooledConnection {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            let mut inner = self.pool.lock().unwrap();
            inner.active_count -= 1;
            if !inner.closed && !client.is_closed() {
                inner.idle.push_back(client);
            } else {
                // Connection is dead or pool is closed — just drop it
                drop(client);
            }
        }
    }
}

// ─── Async Pool ──────────────────────────────────────────────────────────────

struct AsyncPoolInner {
    config: PoolConfig,
    idle: VecDeque<cubrid_tokio::Client>,
    active_count: usize,
    closed: bool,
}

/// An async connection pool for `cubrid_tokio::Client`.
///
/// Uses `tokio::sync::Mutex` for async-safe locking.
/// Connections are returned to the pool when dropped.
pub struct AsyncPool {
    inner: Arc<AsyncMutex<AsyncPoolInner>>,
}

impl AsyncPool {
    /// Create a new async connection pool.
    ///
    /// Pre-creates `min_idle` connections on construction.
    pub async fn new(config: PoolConfig) -> Result<Self, Error> {
        config.validate()?;

        let mut idle = VecDeque::with_capacity(config.max_size);
        for _ in 0..config.min_idle {
            let client = cubrid_tokio::Client::connect(&config.dsn).await?;
            idle.push_back(client);
        }

        Ok(AsyncPool {
            inner: Arc::new(AsyncMutex::new(AsyncPoolInner {
                config,
                idle,
                active_count: 0,
                closed: false,
            })),
        })
    }

    /// Get a connection from the pool.
    ///
    /// If an idle connection is available, it is returned.
    /// If the pool hasn't reached `max_size`, a new connection is created.
    /// Otherwise, returns `Error::PoolExhausted`.
    pub async fn get(&self) -> Result<AsyncPooledConnection, Error> {
        let mut inner = self.inner.lock().await;
        if inner.closed {
            return Err(Error::PoolClosed);
        }

        // Try to get an idle connection
        if let Some(client) = inner.idle.pop_front() {
            inner.active_count += 1;
            return Ok(AsyncPooledConnection {
                client: Some(client),
                pool: Arc::clone(&self.inner),
            });
        }

        // Create a new connection if under max
        let total = inner.active_count + inner.idle.len();
        if total < inner.config.max_size {
            let dsn = inner.config.dsn.clone();
            inner.active_count += 1;
            // Drop lock before connecting
            drop(inner);
            match cubrid_tokio::Client::connect(&dsn).await {
                Ok(client) => Ok(AsyncPooledConnection {
                    client: Some(client),
                    pool: Arc::clone(&self.inner),
                }),
                Err(e) => {
                    let mut inner = self.inner.lock().await;
                    inner.active_count -= 1;
                    Err(e.into())
                }
            }
        } else {
            Err(Error::PoolExhausted {
                max: inner.config.max_size,
            })
        }
    }

    /// Get pool statistics.
    pub async fn status(&self) -> PoolStatus {
        let inner = self.inner.lock().await;
        PoolStatus {
            max_size: inner.config.max_size,
            idle_count: inner.idle.len(),
            active_count: inner.active_count,
            closed: inner.closed,
        }
    }

    /// Close the pool and all idle connections.
    pub async fn close(&self) {
        let mut inner = self.inner.lock().await;
        inner.closed = true;
        for mut client in inner.idle.drain(..) {
            let _ = client.close().await;
        }
    }
}

/// A connection from the async pool.
///
/// Returns to the pool when dropped.
/// Derefs to `cubrid_tokio::Client` for transparent usage.
pub struct AsyncPooledConnection {
    client: Option<cubrid_tokio::Client>,
    pool: Arc<AsyncMutex<AsyncPoolInner>>,
}

impl Deref for AsyncPooledConnection {
    type Target = cubrid_tokio::Client;

    fn deref(&self) -> &Self::Target {
        self.client.as_ref().unwrap()
    }
}

impl DerefMut for AsyncPooledConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.client.as_mut().unwrap()
    }
}

impl Drop for AsyncPooledConnection {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            let pool = Arc::clone(&self.pool);
            if client.is_closed() {
                // Dead connection — just drop
                return;
            }
            // Spawn a task to return the connection to the pool
            tokio::spawn(async move {
                let mut inner = pool.lock().await;
                inner.active_count -= 1;
                if !inner.closed {
                    inner.idle.push_back(client);
                }
            });
        }
    }
}

// ─── Pool Status ─────────────────────────────────────────────────────────────

/// Pool status information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolStatus {
    /// Maximum pool size.
    pub max_size: usize,
    /// Number of idle connections.
    pub idle_count: usize,
    /// Number of active (checked out) connections.
    pub active_count: usize,
    /// Whether the pool is closed.
    pub closed: bool,
}

impl PoolStatus {
    /// Total connections (idle + active).
    pub fn total(&self) -> usize {
        self.idle_count + self.active_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_new() {
        let config = PoolConfig::new("cubrid://dba:@localhost:33000/testdb");
        assert_eq!(config.dsn, "cubrid://dba:@localhost:33000/testdb");
        assert_eq!(config.max_size, 10);
        assert_eq!(config.min_idle, 1);
    }

    #[test]
    fn test_pool_config_builder() {
        let config = PoolConfig::new("cubrid://dba:@localhost:33000/testdb")
            .max_size(20)
            .min_idle(5);
        assert_eq!(config.max_size, 20);
        assert_eq!(config.min_idle, 5);
    }

    #[test]
    fn test_pool_config_default() {
        let config = PoolConfig::default();
        assert!(config.dsn.is_empty());
        assert_eq!(config.max_size, 10);
        assert_eq!(config.min_idle, 1);
    }

    #[test]
    fn test_pool_config_validate_empty_dsn() {
        let config = PoolConfig::default();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("DSN cannot be empty"));
    }

    #[test]
    fn test_pool_config_validate_zero_max() {
        let config = PoolConfig::new("cubrid://dba:@localhost:33000/testdb").max_size(0);
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("max_size must be greater than 0"));
    }

    #[test]
    fn test_pool_config_validate_min_exceeds_max() {
        let config = PoolConfig::new("cubrid://dba:@localhost:33000/testdb")
            .max_size(5)
            .min_idle(10);
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("min_idle cannot exceed max_size"));
    }

    #[test]
    fn test_pool_config_validate_ok() {
        let config = PoolConfig::new("cubrid://dba:@localhost:33000/testdb")
            .max_size(10)
            .min_idle(2);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_pool_config_clone() {
        let config = PoolConfig::new("cubrid://dba:@localhost:33000/testdb").max_size(20);
        let cloned = config.clone();
        assert_eq!(cloned.dsn, config.dsn);
        assert_eq!(cloned.max_size, config.max_size);
    }

    #[test]
    fn test_pool_config_debug() {
        let config = PoolConfig::new("cubrid://dba:@localhost:33000/testdb");
        let debug_str = format!("{config:?}");
        assert!(debug_str.contains("PoolConfig"));
        assert!(debug_str.contains("testdb"));
    }

    #[test]
    fn test_pool_status() {
        let status = PoolStatus {
            max_size: 10,
            idle_count: 3,
            active_count: 2,
            closed: false,
        };
        assert_eq!(status.total(), 5);
        assert!(!status.closed);
    }

    #[test]
    fn test_pool_status_clone() {
        let status = PoolStatus {
            max_size: 10,
            idle_count: 3,
            active_count: 2,
            closed: false,
        };
        let cloned = status;
        assert_eq!(cloned, status);
    }

    #[test]
    fn test_pool_status_debug() {
        let status = PoolStatus {
            max_size: 10,
            idle_count: 0,
            active_count: 0,
            closed: true,
        };
        let debug_str = format!("{status:?}");
        assert!(debug_str.contains("PoolStatus"));
        assert!(debug_str.contains("closed: true"));
    }

    #[test]
    fn test_error_display() {
        let err = Error::PoolExhausted { max: 10 };
        assert_eq!(err.to_string(), "pool exhausted: max 10 connections");

        let err = Error::PoolClosed;
        assert_eq!(err.to_string(), "pool closed");

        let err = Error::InvalidConfig("bad config".to_string());
        assert_eq!(err.to_string(), "invalid config: bad config");
    }

    // Sync pool tests (without real DB — test error handling and config)
    #[test]
    fn test_sync_pool_invalid_config() {
        let config = PoolConfig::default(); // empty DSN
        let result = SyncPool::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_sync_pool_connect_failure() {
        // min_idle=0 so no connections made at construction
        let config = PoolConfig::new("cubrid://dba:@192.0.2.1:33000/testdb").min_idle(0);
        let pool = SyncPool::new(config).unwrap();

        let status = pool.status();
        assert_eq!(status.idle_count, 0);
        assert_eq!(status.active_count, 0);
        assert!(!status.closed);

        // Getting a connection will try to create one — should fail
        let result = pool.get();
        assert!(result.is_err());

        pool.close();
        let status = pool.status();
        assert!(status.closed);
    }

    #[test]
    fn test_sync_pool_closed_get_fails() {
        let config = PoolConfig::new("cubrid://dba:@localhost:33000/testdb").min_idle(0);
        let pool = SyncPool::new(config).unwrap();
        pool.close();

        let result = pool.get();
        assert!(matches!(result, Err(Error::PoolClosed)));
    }

    #[tokio::test]
    async fn test_async_pool_invalid_config() {
        let config = PoolConfig::default();
        let result = AsyncPool::new(config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_async_pool_connect_failure() {
        let config = PoolConfig::new("cubrid://dba:@192.0.2.1:33000/testdb").min_idle(0);
        let pool = AsyncPool::new(config).await.unwrap();

        let status = pool.status().await;
        assert_eq!(status.idle_count, 0);
        assert_eq!(status.active_count, 0);

        // Getting a connection will try to create one — should fail (timeout)
        let result = pool.get().await;
        assert!(result.is_err());

        pool.close().await;
        let status = pool.status().await;
        assert!(status.closed);
    }

    #[tokio::test]
    async fn test_async_pool_closed_get_fails() {
        let config = PoolConfig::new("cubrid://dba:@localhost:33000/testdb").min_idle(0);
        let pool = AsyncPool::new(config).await.unwrap();
        pool.close().await;

        let result = pool.get().await;
        assert!(matches!(result, Err(Error::PoolClosed)));
    }
}
