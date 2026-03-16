//! Integration tests for cubrid-pool.
//!
//! Requires a running CUBRID instance. Set CUBRID_DSN environment variable.
//! Example: CUBRID_DSN=cubrid://dba:@localhost:33000/benchdb

use cubrid_pool::{AsyncPool, Error, PoolConfig, SyncPool};
use cubrid_protocol::value::Value;

fn get_dsn() -> String {
    std::env::var("CUBRID_DSN").unwrap_or_else(|_| {
        eprintln!("CUBRID_DSN not set, skipping integration test");
        std::process::exit(0);
    })
}

// ─── Sync Pool Tests ─────────────────────────────────────────────────────────

#[test]
fn test_sync_pool_create_and_status() {
    let dsn = get_dsn();
    let config = PoolConfig::new(&dsn).max_size(5).min_idle(2);
    let pool = SyncPool::new(config).unwrap();
    let status = pool.status();
    assert_eq!(status.max_size, 5);
    assert_eq!(status.idle_count, 2);
    assert_eq!(status.active_count, 0);
    assert!(!status.closed);
    pool.close();
}

#[test]
fn test_sync_pool_get_and_return() {
    let dsn = get_dsn();
    let config = PoolConfig::new(&dsn).max_size(3).min_idle(1);
    let pool = SyncPool::new(config).unwrap();

    // Get a connection — should move from idle to active
    {
        let mut conn = pool.get().unwrap();
        let status = pool.status();
        assert_eq!(status.active_count, 1);

        // Use the connection
        let rows = conn.query("SELECT 1 + 1 AS result", &[]).unwrap();
        assert_eq!(rows.len(), 1);
    }
    // Connection returned to pool
    let status = pool.status();
    assert_eq!(status.active_count, 0);
    assert!(status.idle_count >= 1);

    pool.close();
}

#[test]
fn test_sync_pool_multiple_connections() {
    let dsn = get_dsn();
    let config = PoolConfig::new(&dsn).max_size(3).min_idle(1);
    let pool = SyncPool::new(config).unwrap();

    let conn1 = pool.get().unwrap();
    let conn2 = pool.get().unwrap();
    let status = pool.status();
    assert_eq!(status.active_count, 2);

    drop(conn1);
    drop(conn2);

    let status = pool.status();
    assert_eq!(status.active_count, 0);
    assert!(status.idle_count >= 2);

    pool.close();
}

#[test]
fn test_sync_pool_exhausted() {
    let dsn = get_dsn();
    let config = PoolConfig::new(&dsn).max_size(2).min_idle(0);
    let pool = SyncPool::new(config).unwrap();

    let _conn1 = pool.get().unwrap();
    let _conn2 = pool.get().unwrap();

    // Third get should fail
    let result = pool.get();
    match result {
        Err(Error::PoolExhausted { max }) => assert_eq!(max, 2),
        Err(e) => panic!("expected PoolExhausted, got: {e}"),
        Ok(_) => panic!("expected error, got Ok"),
    }

    pool.close();
}

#[test]
fn test_sync_pool_close_rejects_new_gets() {
    let dsn = get_dsn();
    let config = PoolConfig::new(&dsn).max_size(3).min_idle(1);
    let pool = SyncPool::new(config).unwrap();
    pool.close();

    let result = pool.get();
    match result {
        Err(Error::PoolClosed) => {}
        Err(e) => panic!("expected PoolClosed, got: {e}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[test]
fn test_sync_pool_query_with_params() {
    let dsn = get_dsn();
    let config = PoolConfig::new(&dsn).max_size(3).min_idle(1);
    let pool = SyncPool::new(config).unwrap();

    let mut conn = pool.get().unwrap();

    // Create table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS pool_test_sync (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))",
        &[],
    )
    .unwrap();

    // Insert
    conn.execute(
        "INSERT INTO pool_test_sync (name) VALUES (?)",
        &[Value::from("pool_sync")],
    )
    .unwrap();

    // Query
    let rows = conn
        .query(
            "SELECT name FROM pool_test_sync WHERE name = ?",
            &[Value::from("pool_sync")],
        )
        .unwrap();
    assert!(!rows.is_empty());

    // Cleanup
    conn.execute("DROP TABLE IF EXISTS pool_test_sync", &[])
        .unwrap();
    drop(conn);
    pool.close();
}

#[test]
fn test_sync_pool_connection_reuse() {
    let dsn = get_dsn();
    let config = PoolConfig::new(&dsn).max_size(1).min_idle(1);
    let pool = SyncPool::new(config).unwrap();

    // Use connection, return it, get it again — should reuse
    {
        let mut conn = pool.get().unwrap();
        conn.query("SELECT 1", &[]).unwrap();
    }
    {
        let mut conn = pool.get().unwrap();
        conn.query("SELECT 2", &[]).unwrap();
    }

    let status = pool.status();
    assert_eq!(status.idle_count, 1);
    assert_eq!(status.active_count, 0);

    pool.close();
}

// ─── Async Pool Tests ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_async_pool_create_and_status() {
    let dsn = get_dsn();
    let config = PoolConfig::new(&dsn).max_size(5).min_idle(2);
    let pool = AsyncPool::new(config).await.unwrap();
    let status = pool.status().await;
    assert_eq!(status.max_size, 5);
    assert_eq!(status.idle_count, 2);
    assert_eq!(status.active_count, 0);
    assert!(!status.closed);
    pool.close().await;
}

#[tokio::test]
async fn test_async_pool_get_and_return() {
    let dsn = get_dsn();
    let config = PoolConfig::new(&dsn).max_size(3).min_idle(1);
    let pool = AsyncPool::new(config).await.unwrap();

    {
        let mut conn = pool.get().await.unwrap();
        let status = pool.status().await;
        assert_eq!(status.active_count, 1);

        let rows = conn.query("SELECT 1 + 1 AS result", &[]).await.unwrap();
        assert_eq!(rows.len(), 1);
    }

    // Small delay for drop to propagate
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let status = pool.status().await;
    assert_eq!(status.active_count, 0);
    assert!(status.idle_count >= 1);

    pool.close().await;
}

#[tokio::test]
async fn test_async_pool_multiple_connections() {
    let dsn = get_dsn();
    let config = PoolConfig::new(&dsn).max_size(3).min_idle(1);
    let pool = AsyncPool::new(config).await.unwrap();

    let conn1 = pool.get().await.unwrap();
    let conn2 = pool.get().await.unwrap();
    let status = pool.status().await;
    assert_eq!(status.active_count, 2);

    drop(conn1);
    drop(conn2);

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let status = pool.status().await;
    assert_eq!(status.active_count, 0);

    pool.close().await;
}

#[tokio::test]
async fn test_async_pool_exhausted() {
    let dsn = get_dsn();
    let config = PoolConfig::new(&dsn).max_size(2).min_idle(0);
    let pool = AsyncPool::new(config).await.unwrap();

    let _conn1 = pool.get().await.unwrap();
    let _conn2 = pool.get().await.unwrap();

    let result = pool.get().await;
    match result {
        Err(Error::PoolExhausted { max }) => assert_eq!(max, 2),
        Err(e) => panic!("expected PoolExhausted, got: {e}"),
        Ok(_) => panic!("expected error, got Ok"),
    }

    pool.close().await;
}

#[tokio::test]
async fn test_async_pool_close_rejects_new_gets() {
    let dsn = get_dsn();
    let config = PoolConfig::new(&dsn).max_size(3).min_idle(1);
    let pool = AsyncPool::new(config).await.unwrap();
    pool.close().await;

    let result = pool.get().await;
    match result {
        Err(Error::PoolClosed) => {}
        Err(e) => panic!("expected PoolClosed, got: {e}"),
        Ok(_) => panic!("expected error, got Ok"),
    }
}

#[tokio::test]
async fn test_async_pool_query_with_params() {
    let dsn = get_dsn();
    let config = PoolConfig::new(&dsn).max_size(3).min_idle(1);
    let pool = AsyncPool::new(config).await.unwrap();

    let mut conn = pool.get().await.unwrap();

    conn.execute(
        "CREATE TABLE IF NOT EXISTS pool_test_async (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))",
        &[],
    )
    .await
    .unwrap();

    conn.execute(
        "INSERT INTO pool_test_async (name) VALUES (?)",
        &[Value::from("pool_async")],
    )
    .await
    .unwrap();

    let rows = conn
        .query(
            "SELECT name FROM pool_test_async WHERE name = ?",
            &[Value::from("pool_async")],
        )
        .await
        .unwrap();
    assert!(!rows.is_empty());

    conn.execute("DROP TABLE IF EXISTS pool_test_async", &[])
        .await
        .unwrap();
    drop(conn);
    pool.close().await;
}
