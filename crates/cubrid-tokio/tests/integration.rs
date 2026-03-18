//! Async integration tests for cubrid-tokio.
//!
//! Requires a running CUBRID instance at localhost:33000.
//! Set `CUBRID_TEST_URL` to override the default DSN.
//!
//! Run with: `cargo test --test integration_async`

use cubrid_protocol::value::Value;
use cubrid_tokio::Client;

fn test_dsn() -> String {
    std::env::var("CUBRID_TEST_URL")
        .unwrap_or_else(|_| "cubrid://dba:@localhost:33000/benchdb".to_string())
}

fn test_table(suffix: &str) -> String {
    format!("test_async_{suffix}")
}

// ─── Connection Tests ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_async_connect_and_ping() {
    let mut client = Client::connect(&test_dsn()).await.expect("connect");
    let version = client.ping().await.expect("ping");
    assert!(!version.is_empty());
    assert!(version.chars().any(|c| c.is_ascii_digit()));
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_connect_and_close() {
    let mut client = Client::connect(&test_dsn()).await.expect("connect");
    assert!(!client.is_closed());
    client.close().await.expect("close");
    assert!(client.is_closed());
}

// ─── DDL/DML Tests ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_async_create_insert_query_drop() {
    let table = test_table("crud");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;

    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .await
        .expect("create");

    let affected = client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::from("async_test")],
        )
        .await
        .expect("insert");
    assert_eq!(affected, 1);

    let result = client
        .query(&format!("SELECT id, name FROM {table}"), &[])
        .await
        .expect("query");
    assert_eq!(result.len(), 1);

    match &result.rows[0][1] {
        Value::String(v) => assert_eq!(v, "async_test"),
        other => panic!("expected String, got: {other:?}"),
    }

    client
        .execute(&format!("DROP TABLE {table}"), &[])
        .await
        .expect("drop");
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_select_expression() {
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let result = client.query("SELECT 2 + 3", &[]).await.expect("query");
    assert_eq!(result.len(), 1);
    match &result.rows[0][0] {
        Value::Int(v) => assert_eq!(*v, 5),
        Value::Long(v) => assert_eq!(*v, 5),
        other => panic!("expected numeric 5, got: {other:?}"),
    }

    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_transaction_commit() {
    let table = test_table("tx_commit");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .await
        .expect("create");

    client.set_auto_commit(false);
    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::from("committed_async")],
        )
        .await
        .expect("insert");
    client.commit().await.expect("commit");

    let result = client
        .query(&format!("SELECT name FROM {table}"), &[])
        .await
        .expect("query");
    assert_eq!(result.len(), 1);

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_transaction_rollback() {
    let table = test_table("tx_rollback");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .await
        .expect("create");

    // Insert with auto-commit on
    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::from("keep_async")],
        )
        .await
        .expect("insert keep");

    // Insert and rollback
    client.set_auto_commit(false);
    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::from("rollback_async")],
        )
        .await
        .expect("insert rollback");
    client.rollback().await.expect("rollback");

    client.set_auto_commit(true);
    let result = client
        .query(&format!("SELECT COUNT(*) FROM {table}"), &[])
        .await
        .expect("count");
    match &result.rows[0][0] {
        Value::Long(v) => assert_eq!(*v, 1),
        Value::Int(v) => assert_eq!(*v, 1),
        other => panic!("expected numeric 1, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_null_handling() {
    let table = test_table("nulls");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .await
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::Null],
        )
        .await
        .expect("insert null");

    let result = client
        .query(&format!("SELECT name FROM {table}"), &[])
        .await
        .expect("query");
    assert!(matches!(&result.rows[0][0], Value::Null));

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_large_result_set() {
    let table = test_table("large_rs");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)"),
            &[],
        )
        .await
        .expect("create");

    // Insert 150 rows
    for i in 1..=150 {
        client
            .execute(
                &format!("INSERT INTO {table} (val) VALUES (?)"),
                &[Value::Int(i)],
            )
            .await
            .expect("insert");
    }

    let result = client
        .query(&format!("SELECT * FROM {table} ORDER BY id"), &[])
        .await
        .expect("query");
    assert_eq!(result.len(), 150);

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_multiple_connections() {
    let mut c1 = Client::connect(&test_dsn()).await.expect("connect 1");
    let mut c2 = Client::connect(&test_dsn()).await.expect("connect 2");

    let v1 = c1.ping().await.expect("ping 1");
    let v2 = c2.ping().await.expect("ping 2");
    assert_eq!(v1, v2);

    c1.close().await.expect("close 1");
    c2.close().await.expect("close 2");
}

#[tokio::test]
async fn test_async_sql_error() {
    let mut client = Client::connect(&test_dsn()).await.expect("connect");
    let result = client
        .execute("SELECT * FROM nonexistent_table_xyz_async", &[])
        .await;
    assert!(result.is_err());
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_last_insert_id() {
    let table = test_table("last_id");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .await
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::from("async_id")],
        )
        .await
        .expect("insert");

    let last_id = client.last_insert_id().await.expect("last_insert_id");
    assert!(!last_id.is_empty());

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

// ─── Additional Connection Tests ────────────────────────────────────────────

#[tokio::test]
async fn test_async_connect_invalid_dsn() {
    let result = Client::connect("cubrid://dba:@192.0.2.1:33000/nonexist").await;
    assert!(result.is_err(), "should fail to connect to invalid host");
}

#[tokio::test]
async fn test_async_double_close() {
    let mut client = Client::connect(&test_dsn()).await.expect("connect");
    client.close().await.expect("first close");
    client
        .close()
        .await
        .expect("second close should be idempotent");
}

#[tokio::test]
async fn test_async_operations_after_close() {
    let mut client = Client::connect(&test_dsn()).await.expect("connect");
    client.close().await.expect("close");
    let result = client.execute("SELECT 1", &[]).await;
    assert!(result.is_err(), "execute after close should fail");
}

// ─── Additional DDL/DML Tests ───────────────────────────────────────────────

#[tokio::test]
async fn test_async_create_and_drop_table() {
    let table = test_table("create_drop");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;

    client
        .execute(
            &format!(
                "CREATE TABLE {table} (
                    id INT AUTO_INCREMENT PRIMARY KEY,
                    name VARCHAR(100),
                    age INT
                )"
            ),
            &[],
        )
        .await
        .expect("CREATE TABLE");

    client
        .execute(&format!("DROP TABLE {table}"), &[])
        .await
        .expect("DROP TABLE");

    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_insert_multiple_and_count() {
    let table = test_table("multi_insert");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)"),
            &[],
        )
        .await
        .expect("create");

    for i in 1..=10 {
        client
            .execute(
                &format!("INSERT INTO {table} (val) VALUES (?)"),
                &[Value::Int(i)],
            )
            .await
            .expect("insert");
    }

    let result = client
        .query(&format!("SELECT COUNT(*) FROM {table}"), &[])
        .await
        .expect("count");
    assert_eq!(result.len(), 1);
    match &result.rows[0][0] {
        Value::Long(v) => assert_eq!(*v, 10),
        Value::Int(v) => assert_eq!(*v, 10),
        other => panic!("expected numeric, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_update_rows() {
    let table = test_table("update");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .await
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::from("before")],
        )
        .await
        .expect("insert");

    let affected = client
        .execute(
            &format!("UPDATE {table} SET name = ? WHERE name = ?"),
            &[Value::from("after"), Value::from("before")],
        )
        .await
        .expect("update");
    assert_eq!(affected, 1);

    let result = client
        .query(&format!("SELECT name FROM {table}"), &[])
        .await
        .expect("query");
    match &result.rows[0][0] {
        Value::String(v) => assert_eq!(v, "after"),
        other => panic!("expected String, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_delete_rows() {
    let table = test_table("delete");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .await
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES ('a'), ('b'), ('c')"),
            &[],
        )
        .await
        .expect("insert 3");

    let affected = client
        .execute(
            &format!("DELETE FROM {table} WHERE name = ?"),
            &[Value::from("b")],
        )
        .await
        .expect("delete");
    assert_eq!(affected, 1);

    let result = client
        .query(&format!("SELECT COUNT(*) FROM {table}"), &[])
        .await
        .expect("count");
    match &result.rows[0][0] {
        Value::Long(v) => assert_eq!(*v, 2),
        Value::Int(v) => assert_eq!(*v, 2),
        other => panic!("expected numeric, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

// ─── Type Tests ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_async_numeric_types() {
    let table = test_table("numerics");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!(
                "CREATE TABLE {table} (
                    si SMALLINT,
                    i INT,
                    bi BIGINT,
                    f FLOAT,
                    d DOUBLE
                )"
            ),
            &[],
        )
        .await
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (si, i, bi, f, d) VALUES (?, ?, ?, ?, ?)"),
            &[
                Value::Short(32000),
                Value::Int(2_000_000),
                Value::Long(9_000_000_000),
                Value::Float(std::f32::consts::PI),
                Value::Double(std::f64::consts::E),
            ],
        )
        .await
        .expect("insert");

    let result = client
        .query(&format!("SELECT si, i, bi, f, d FROM {table}"), &[])
        .await
        .expect("query");
    assert_eq!(result.len(), 1);
    let row = &result.rows[0];
    assert!(!format!("{:?}", row[0]).is_empty());
    assert!(!format!("{:?}", row[1]).is_empty());
    assert!(!format!("{:?}", row[2]).is_empty());
    assert!(!format!("{:?}", row[3]).is_empty());
    assert!(!format!("{:?}", row[4]).is_empty());

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_string_types() {
    let table = test_table("strings");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!(
                "CREATE TABLE {table} (
                    c CHAR(10),
                    vc VARCHAR(255),
                    s STRING
                )"
            ),
            &[],
        )
        .await
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (c, vc, s) VALUES (?, ?, ?)"),
            &[
                Value::from("hello"),
                Value::from("world of CUBRID"),
                Value::from("This is a longer string value for testing."),
            ],
        )
        .await
        .expect("insert");

    let result = client
        .query(&format!("SELECT c, vc, s FROM {table}"), &[])
        .await
        .expect("query");
    assert_eq!(result.len(), 1);
    let row = &result.rows[0];
    match &row[0] {
        Value::String(v) => assert!(v.starts_with("hello"), "CHAR value: {v:?}"),
        other => panic!("expected String, got: {other:?}"),
    }
    match &row[1] {
        Value::String(v) => assert_eq!(v, "world of CUBRID"),
        other => panic!("expected String, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_date_time_types() {
    let table = test_table("datetime");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!(
                "CREATE TABLE {table} (
                    d DATE,
                    t TIME,
                    ts TIMESTAMP,
                    dt DATETIME
                )"
            ),
            &[],
        )
        .await
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (d, t, ts, dt) VALUES (?, ?, ?, ?)"),
            &[
                Value::Date {
                    year: 2024,
                    month: 6,
                    day: 15,
                },
                Value::Time {
                    hour: 14,
                    minute: 30,
                    second: 45,
                },
                Value::Timestamp {
                    year: 2024,
                    month: 6,
                    day: 15,
                    hour: 14,
                    minute: 30,
                    second: 45,
                },
                Value::Datetime {
                    year: 2024,
                    month: 6,
                    day: 15,
                    hour: 14,
                    minute: 30,
                    second: 45,
                    ms: 123,
                },
            ],
        )
        .await
        .expect("insert");

    let result = client
        .query(&format!("SELECT d, t, ts, dt FROM {table}"), &[])
        .await
        .expect("query");
    assert_eq!(result.len(), 1);
    for (i, val) in result.rows[0].iter().enumerate() {
        assert!(
            !matches!(val, Value::Null),
            "column {i} should not be null: {val:?}"
        );
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

// ─── Query Edge Cases ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_async_empty_result_set() {
    let table = test_table("empty_rs");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .await
        .expect("create");

    let result = client
        .query(&format!("SELECT * FROM {table}"), &[])
        .await
        .expect("query");
    assert!(result.is_empty());
    assert_eq!(result.len(), 0);

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_select_multiple_expressions() {
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let result = client
        .query("SELECT 1, 'hello', 3.14", &[])
        .await
        .expect("query");
    assert_eq!(result.len(), 1);
    assert_eq!(result.rows[0].len(), 3);

    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_column_metadata() {
    let table = test_table("col_meta");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!(
                "CREATE TABLE {table} (
                    id INT AUTO_INCREMENT PRIMARY KEY,
                    name VARCHAR(100) NOT NULL,
                    score DOUBLE
                )"
            ),
            &[],
        )
        .await
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name, score) VALUES (?, ?)"),
            &[Value::from("test"), Value::Double(95.5)],
        )
        .await
        .expect("insert");

    let result = client
        .query(&format!("SELECT id, name, score FROM {table}"), &[])
        .await
        .expect("query");
    let names = result.column_names();
    assert_eq!(names.len(), 3);
    assert!(names.iter().any(|n| n.eq_ignore_ascii_case("id")));
    assert!(names.iter().any(|n| n.eq_ignore_ascii_case("name")));
    assert!(names.iter().any(|n| n.eq_ignore_ascii_case("score")));

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_special_characters_in_strings() {
    let table = test_table("special_chars");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val VARCHAR(500))"),
            &[],
        )
        .await
        .expect("create");

    let special = "it's a \"test\" with backslash \\ and 한글 and emoji 🎉";
    client
        .execute(
            &format!("INSERT INTO {table} (val) VALUES (?)"),
            &[Value::from(special)],
        )
        .await
        .expect("insert special chars");

    let result = client
        .query(&format!("SELECT val FROM {table}"), &[])
        .await
        .expect("query");
    assert_eq!(result.len(), 1);
    match &result.rows[0][0] {
        Value::String(v) => {
            assert!(v.contains("한글"), "should contain Korean: {v:?}");
        }
        other => panic!("expected String, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

// ─── Auto-commit / Proto Version ────────────────────────────────────────────

#[tokio::test]
async fn test_async_auto_commit_toggle() {
    let mut client = Client::connect(&test_dsn()).await.expect("connect");
    assert!(client.auto_commit(), "default should be auto-commit on");

    client.set_auto_commit(false);
    assert!(!client.auto_commit());

    client.set_auto_commit(true);
    assert!(client.auto_commit());

    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_proto_version() {
    let client = Client::connect(&test_dsn()).await.expect("connect");
    let version = client.proto_version();
    assert!(
        version >= 0,
        "proto version should be non-negative: {version}"
    );
}

// ─── Prepared Statement Tests ───────────────────────────────────────────────

#[tokio::test]
async fn test_async_prepared_statement_execute() {
    let table = test_table("prep_exec");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)"),
            &[],
        )
        .await
        .expect("create");

    let mut stmt = client
        .prepare(&format!("INSERT INTO {table} (val) VALUES (?)"))
        .await
        .expect("prepare");

    for i in 1..=5 {
        let affected = stmt
            .execute(&mut client, &[Value::Int(i * 10)])
            .await
            .expect("exec");
        assert_eq!(affected, 1);
    }

    stmt.close(&mut client).await.expect("close stmt");

    let result = client
        .query(&format!("SELECT COUNT(*) FROM {table}"), &[])
        .await
        .expect("count");
    match &result.rows[0][0] {
        Value::Long(v) => assert_eq!(*v, 5),
        Value::Int(v) => assert_eq!(*v, 5),
        other => panic!("expected numeric, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_prepared_statement_query() {
    let table = test_table("prep_query");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .await
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES ('alpha'), ('beta'), ('gamma')"),
            &[],
        )
        .await
        .expect("insert 3");

    let mut stmt = client
        .prepare(&format!("SELECT id, name FROM {table} WHERE name = ?"))
        .await
        .expect("prepare");

    let result = stmt
        .query_with(&mut client, &[Value::from("beta")])
        .await
        .expect("query_with");
    assert_eq!(result.len(), 1);
    match &result.rows[0][1] {
        Value::String(v) => assert_eq!(v, "beta"),
        other => panic!("expected String, got: {other:?}"),
    }

    stmt.close(&mut client).await.expect("close stmt");
    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

// ─── Prepared Statement: Empty Params (FC=3 Path) ──────────────────────────

#[tokio::test]
async fn test_async_prepared_statement_execute_no_params() {
    let table = test_table("prep_noparams");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)"),
            &[],
        )
        .await
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (val) VALUES (10), (20), (30)"),
            &[],
        )
        .await
        .expect("insert 3");

    // Prepare INSERT with no placeholders — execute with empty params → FC=3
    let mut stmt = client
        .prepare(&format!("INSERT INTO {table} (val) VALUES (99)"))
        .await
        .expect("prepare");
    let affected = stmt
        .execute(&mut client, &[])
        .await
        .expect("exec no params");
    assert_eq!(affected, 1);
    stmt.close(&mut client).await.expect("close stmt");

    let result = client
        .query(&format!("SELECT COUNT(*) FROM {table}"), &[])
        .await
        .expect("count");
    match &result.rows[0][0] {
        Value::Long(v) => assert_eq!(*v, 4),
        Value::Int(v) => assert_eq!(*v, 4),
        other => panic!("expected numeric, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_prepared_statement_query_no_params() {
    let table = test_table("prep_q_noparams");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .await
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES ('alpha'), ('beta'), ('gamma')"),
            &[],
        )
        .await
        .expect("insert 3");

    // Prepare SELECT with no placeholders — query_with with empty params → FC=3
    let mut stmt = client
        .prepare(&format!("SELECT id, name FROM {table} ORDER BY id"))
        .await
        .expect("prepare");
    let result = stmt
        .query_with(&mut client, &[])
        .await
        .expect("query no params");
    assert_eq!(result.len(), 3);
    stmt.close(&mut client).await.expect("close stmt");

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_prepared_statement_query_closed_error() {
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let mut stmt = client.prepare("SELECT 1").await.expect("prepare");
    stmt.close(&mut client).await.expect("close stmt");

    // query_with after close should fail
    let result = stmt.query_with(&mut client, &[]).await;
    assert!(result.is_err(), "query_with after close should fail");

    // execute after close should fail
    let result2 = stmt.execute(&mut client, &[]).await;
    assert!(result2.is_err(), "execute after close should fail");

    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_prepared_statement_large_result_fetch() {
    let table = test_table("prep_large");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)"),
            &[],
        )
        .await
        .expect("create");

    // Insert 200 rows to exercise fetch_remaining (DEFAULT_FETCH_SIZE=100)
    for i in 1..=200 {
        client
            .execute(
                &format!("INSERT INTO {table} (val) VALUES (?)"),
                &[Value::Int(i)],
            )
            .await
            .expect("insert");
    }

    // Prepare SELECT to get all rows — should trigger fetch_remaining
    let mut stmt = client
        .prepare(&format!("SELECT id, val FROM {table} ORDER BY id"))
        .await
        .expect("prepare");
    let result = stmt
        .query_with(&mut client, &[])
        .await
        .expect("query large");
    assert_eq!(
        result.len(),
        200,
        "should fetch all 200 rows via prepared stmt"
    );
    stmt.close(&mut client).await.expect("close stmt");

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_statement_close_already_closed() {
    let table = test_table("async_stmt_close_idem");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)"),
            &[],
        )
        .await
        .expect("create");

    let mut stmt = client
        .prepare(&format!("SELECT * FROM {table}"))
        .await
        .expect("prepare");
    stmt.close(&mut client).await.expect("first close");
    // Second close should be a no-op
    stmt.close(&mut client)
        .await
        .expect("second close should be ok");

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_last_insert_id_with_value() {
    let table = test_table("async_last_id_val");
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let _ = client
        .execute(&format!("DROP TABLE IF EXISTS {table}"), &[])
        .await;
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val VARCHAR(50))"),
            &[],
        )
        .await
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (val) VALUES (?)"),
            &[Value::String("test".to_string())],
        )
        .await
        .expect("insert");

    let id = client.last_insert_id().await.expect("last_insert_id");
    assert!(!id.is_empty(), "should have a non-empty last insert id");
    let _: i64 = id.parse().expect("last_insert_id should be numeric");

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]).await;
    client.close().await.expect("close");
}

#[tokio::test]
async fn test_async_last_insert_id_no_insert() {
    let mut client = Client::connect(&test_dsn()).await.expect("connect");

    let id = client.last_insert_id().await.expect("last_insert_id");
    assert!(id.is_empty() || id == "0", "expected empty or 0, got: {id}");

    client.close().await.expect("close");
}
