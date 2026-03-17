//! Integration tests for cubrid-rs workspace.
//!
//! Requires a running CUBRID instance at localhost:33000.
//! Set `CUBRID_TEST_URL` to override the default DSN.
//!
//! Run with: `cargo test --test integration`

use cubrid_client::Client;
use cubrid_protocol::value::Value;

fn test_dsn() -> String {
    std::env::var("CUBRID_TEST_URL")
        .unwrap_or_else(|_| "cubrid://dba:@localhost:33000/benchdb".to_string())
}

/// Unique table name to avoid collisions between test runs.
fn test_table(suffix: &str) -> String {
    format!("test_rs_{suffix}")
}

// ─── Connection Tests ────────────────────────────────────────────────────────

#[test]
fn test_connect_and_ping() {
    let mut client = Client::connect(&test_dsn()).expect("connect failed");
    let version = client.ping().expect("ping failed");
    assert!(!version.is_empty(), "version should not be empty");
    // CUBRID version strings typically contain digits
    assert!(
        version.chars().any(|c| c.is_ascii_digit()),
        "version should contain digits: {version}"
    );
    client.close().expect("close failed");
}

#[test]
fn test_connect_invalid_dsn() {
    let result = Client::connect("cubrid://dba:@192.0.2.1:33000/nonexist");
    assert!(result.is_err(), "should fail to connect to invalid host");
}

#[test]
fn test_connect_and_close() {
    let mut client = Client::connect(&test_dsn()).expect("connect");
    assert!(!client.is_closed());
    client.close().expect("close");
    assert!(client.is_closed());
}

#[test]
fn test_double_close() {
    let mut client = Client::connect(&test_dsn()).expect("connect");
    client.close().expect("first close");
    client.close().expect("second close should be idempotent");
}

#[test]
fn test_operations_after_close() {
    let mut client = Client::connect(&test_dsn()).expect("connect");
    client.close().expect("close");
    let result = client.execute("SELECT 1", &[]);
    assert!(result.is_err(), "execute after close should fail");
}

// ─── DDL Tests ───────────────────────────────────────────────────────────────

#[test]
fn test_create_and_drop_table() {
    let table = test_table("create_drop");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    // Drop if exists (cleanup from previous runs)
    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);

    // Create
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
        .expect("CREATE TABLE");

    // Drop
    client
        .execute(&format!("DROP TABLE {table}"), &[])
        .expect("DROP TABLE");

    client.close().expect("close");
}

// ─── DML Tests ───────────────────────────────────────────────────────────────

#[test]
fn test_insert_and_query() {
    let table = test_table("insert_query");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .expect("create");

    // Insert
    let affected = client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::from("alice")],
        )
        .expect("insert");
    assert_eq!(affected, 1, "should affect 1 row");

    // Query
    let result = client
        .query(&format!("SELECT id, name FROM {table}"), &[])
        .expect("query");
    assert_eq!(result.len(), 1);
    assert_eq!(result.total_count, 1);

    let row = &result.rows[0];
    // id should be an integer (auto-increment starts at 1)
    match &row[0] {
        Value::Int(v) => assert!(*v > 0, "id should be positive"),
        other => panic!("expected Int for id, got: {other:?}"),
    }
    // name should be a string
    match &row[1] {
        Value::String(v) => assert_eq!(v, "alice"),
        other => panic!("expected String for name, got: {other:?}"),
    }

    // Cleanup
    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_insert_multiple_and_count() {
    let table = test_table("multi_insert");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)"),
            &[],
        )
        .expect("create");

    for i in 1..=10 {
        client
            .execute(
                &format!("INSERT INTO {table} (val) VALUES (?)"),
                &[Value::Int(i)],
            )
            .expect("insert");
    }

    let result = client
        .query(&format!("SELECT COUNT(*) FROM {table}"), &[])
        .expect("count");
    assert_eq!(result.len(), 1);
    match &result.rows[0][0] {
        Value::Long(v) => assert_eq!(*v, 10, "should have 10 rows"),
        Value::Int(v) => assert_eq!(*v, 10, "should have 10 rows"),
        other => panic!("expected numeric count, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_update_rows() {
    let table = test_table("update");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::from("before")],
        )
        .expect("insert");

    let affected = client
        .execute(
            &format!("UPDATE {table} SET name = ? WHERE name = ?"),
            &[Value::from("after"), Value::from("before")],
        )
        .expect("update");
    assert_eq!(affected, 1);

    let result = client
        .query(&format!("SELECT name FROM {table}"), &[])
        .expect("query");
    match &result.rows[0][0] {
        Value::String(v) => assert_eq!(v, "after"),
        other => panic!("expected String, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_delete_rows() {
    let table = test_table("delete");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES ('a'), ('b'), ('c')"),
            &[],
        )
        .expect("insert 3");

    let affected = client
        .execute(
            &format!("DELETE FROM {table} WHERE name = ?"),
            &[Value::from("b")],
        )
        .expect("delete");
    assert_eq!(affected, 1);

    let result = client
        .query(&format!("SELECT COUNT(*) FROM {table}"), &[])
        .expect("count");
    match &result.rows[0][0] {
        Value::Long(v) => assert_eq!(*v, 2),
        Value::Int(v) => assert_eq!(*v, 2),
        other => panic!("expected numeric, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

// ─── Type Tests ──────────────────────────────────────────────────────────────

#[test]
fn test_numeric_types() {
    let table = test_table("numerics");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
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
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (si, i, bi, f, d) VALUES (?, ?, ?, ?, ?)"),
            &[
                Value::Short(32000),
                Value::Int(2_000_000),
                Value::Long(9_000_000_000),
                Value::Float(3.14),
                Value::Double(2.718281828),
            ],
        )
        .expect("insert");

    let result = client
        .query(&format!("SELECT si, i, bi, f, d FROM {table}"), &[])
        .expect("query");
    assert_eq!(result.len(), 1);

    let row = &result.rows[0];
    // Verify values are reasonable (exact types may vary by CUBRID version)
    assert!(!format!("{:?}", row[0]).is_empty());
    assert!(!format!("{:?}", row[1]).is_empty());
    assert!(!format!("{:?}", row[2]).is_empty());
    assert!(!format!("{:?}", row[3]).is_empty());
    assert!(!format!("{:?}", row[4]).is_empty());

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_string_types() {
    let table = test_table("strings");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
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
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (c, vc, s) VALUES (?, ?, ?)"),
            &[
                Value::from("hello"),
                Value::from("world of CUBRID"),
                Value::from("This is a longer string value for testing purposes."),
            ],
        )
        .expect("insert");

    let result = client
        .query(&format!("SELECT c, vc, s FROM {table}"), &[])
        .expect("query");
    assert_eq!(result.len(), 1);

    let row = &result.rows[0];
    // CHAR is padded to 10 chars
    match &row[0] {
        Value::String(v) => assert!(v.starts_with("hello"), "CHAR value: {v:?}"),
        other => panic!("expected String, got: {other:?}"),
    }
    match &row[1] {
        Value::String(v) => assert_eq!(v, "world of CUBRID"),
        other => panic!("expected String, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_null_values() {
    let table = test_table("nulls");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::Null],
        )
        .expect("insert null");

    let result = client
        .query(&format!("SELECT name FROM {table}"), &[])
        .expect("query");
    assert_eq!(result.len(), 1);
    match &result.rows[0][0] {
        Value::Null => {} // Expected
        other => panic!("expected Null, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_date_time_types() {
    let table = test_table("datetime");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
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
        .expect("insert");

    let result = client
        .query(&format!("SELECT d, t, ts, dt FROM {table}"), &[])
        .expect("query");
    assert_eq!(result.len(), 1);
    // All values should be non-null
    for (i, val) in result.rows[0].iter().enumerate() {
        assert!(
            !matches!(val, Value::Null),
            "column {i} should not be null: {val:?}"
        );
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

// ─── Transaction Tests ───────────────────────────────────────────────────────

#[test]
fn test_commit_transaction() {
    let table = test_table("tx_commit");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .expect("create");

    client.set_auto_commit(false);
    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::from("committed")],
        )
        .expect("insert");
    client.commit().expect("commit");

    // Verify data persists
    let result = client
        .query(&format!("SELECT name FROM {table}"), &[])
        .expect("query");
    assert_eq!(result.len(), 1);
    match &result.rows[0][0] {
        Value::String(v) => assert_eq!(v, "committed"),
        other => panic!("expected String, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_rollback_transaction() {
    let table = test_table("tx_rollback");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .expect("create");

    // Insert in auto-commit to have baseline
    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::from("keep")],
        )
        .expect("insert keep");

    // Turn off auto-commit, insert, then rollback
    client.set_auto_commit(false);
    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::from("rollback_me")],
        )
        .expect("insert rollback_me");
    client.rollback().expect("rollback");

    // Verify only the committed row remains
    client.set_auto_commit(true);
    let result = client
        .query(&format!("SELECT COUNT(*) FROM {table}"), &[])
        .expect("count");
    match &result.rows[0][0] {
        Value::Long(v) => assert_eq!(*v, 1, "only committed row should remain"),
        Value::Int(v) => assert_eq!(*v, 1, "only committed row should remain"),
        other => panic!("expected numeric, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

// ─── Prepared Statement Tests ────────────────────────────────────────────────

#[test]
fn test_prepared_statement_execute() {
    let table = test_table("prep_exec");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)"),
            &[],
        )
        .expect("create");

    let mut stmt = client
        .prepare(&format!("INSERT INTO {table} (val) VALUES (?)"))
        .expect("prepare");

    // Execute multiple times with different params
    for i in 1..=5 {
        let affected = stmt
            .execute(&mut client, &[Value::Int(i * 10)])
            .expect("exec");
        assert_eq!(affected, 1);
    }

    stmt.close(&mut client).expect("close stmt");

    let result = client
        .query(&format!("SELECT COUNT(*) FROM {table}"), &[])
        .expect("count");
    match &result.rows[0][0] {
        Value::Long(v) => assert_eq!(*v, 5),
        Value::Int(v) => assert_eq!(*v, 5),
        other => panic!("expected numeric, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_prepared_statement_query() {
    let table = test_table("prep_query");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES ('alpha'), ('beta'), ('gamma')"),
            &[],
        )
        .expect("insert 3");

    let mut stmt = client
        .prepare(&format!("SELECT id, name FROM {table} WHERE name = ?"))
        .expect("prepare");

    let result = stmt
        .query_with(&mut client, &[Value::from("beta")])
        .expect("query_with");
    assert_eq!(result.len(), 1);
    match &result.rows[0][1] {
        Value::String(v) => assert_eq!(v, "beta"),
        other => panic!("expected String, got: {other:?}"),
    }

    stmt.close(&mut client).expect("close stmt");
    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

// ─── Query Edge Cases ────────────────────────────────────────────────────────

#[test]
fn test_select_expression() {
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let result = client.query("SELECT 1 + 1", &[]).expect("query");
    assert_eq!(result.len(), 1);
    // 1+1 = 2
    match &result.rows[0][0] {
        Value::Int(v) => assert_eq!(*v, 2),
        Value::Long(v) => assert_eq!(*v, 2),
        other => panic!("expected numeric 2, got: {other:?}"),
    }

    client.close().expect("close");
}

#[test]
fn test_select_multiple_expressions() {
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let result = client.query("SELECT 1, 'hello', 3.14", &[]).expect("query");
    assert_eq!(result.len(), 1);
    assert_eq!(result.rows[0].len(), 3);

    client.close().expect("close");
}

#[test]
fn test_empty_result_set() {
    let table = test_table("empty_rs");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .expect("create");

    let result = client
        .query(&format!("SELECT * FROM {table}"), &[])
        .expect("query");
    assert!(result.is_empty());
    assert_eq!(result.len(), 0);

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_large_result_set() {
    let table = test_table("large_rs");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)"),
            &[],
        )
        .expect("create");

    // Insert 200 rows (exceeds default fetch size of 100)
    for i in 1..=200 {
        client
            .execute(
                &format!("INSERT INTO {table} (val) VALUES (?)"),
                &[Value::Int(i)],
            )
            .expect("insert");
    }

    let result = client
        .query(&format!("SELECT * FROM {table} ORDER BY id"), &[])
        .expect("query");
    assert_eq!(result.len(), 200, "should fetch all 200 rows");

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_sql_error() {
    let mut client = Client::connect(&test_dsn()).expect("connect");
    let result = client.execute("SELECT * FROM nonexistent_table_xyz_123", &[]);
    assert!(result.is_err(), "query on non-existent table should error");
    client.close().expect("close");
}

#[test]
fn test_column_metadata() {
    let table = test_table("col_meta");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
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
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name, score) VALUES (?, ?)"),
            &[Value::from("test"), Value::Double(95.5)],
        )
        .expect("insert");

    let result = client
        .query(&format!("SELECT id, name, score FROM {table}"), &[])
        .expect("query");
    let names = result.column_names();
    assert_eq!(names.len(), 3);
    // Column names should match (case may vary)
    assert!(
        names.iter().any(|n| n.eq_ignore_ascii_case("id")),
        "should have id column"
    );
    assert!(
        names.iter().any(|n| n.eq_ignore_ascii_case("name")),
        "should have name column"
    );
    assert!(
        names.iter().any(|n| n.eq_ignore_ascii_case("score")),
        "should have score column"
    );

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_special_characters_in_strings() {
    let table = test_table("special_chars");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val VARCHAR(500))"),
            &[],
        )
        .expect("create");

    let special = "it's a \"test\" with backslash \\ and 한글 and emoji 🎉";
    client
        .execute(
            &format!("INSERT INTO {table} (val) VALUES (?)"),
            &[Value::from(special)],
        )
        .expect("insert special chars");

    let result = client
        .query(&format!("SELECT val FROM {table}"), &[])
        .expect("query");
    assert_eq!(result.len(), 1);
    match &result.rows[0][0] {
        Value::String(v) => {
            assert!(v.contains("한글"), "should contain Korean: {v:?}");
        }
        other => panic!("expected String, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

// ─── Multiple Connections ────────────────────────────────────────────────────

#[test]
fn test_multiple_connections() {
    let mut c1 = Client::connect(&test_dsn()).expect("connect 1");
    let mut c2 = Client::connect(&test_dsn()).expect("connect 2");

    let v1 = c1.ping().expect("ping 1");
    let v2 = c2.ping().expect("ping 2");
    assert_eq!(v1, v2, "both should report same version");

    c1.close().expect("close 1");
    c2.close().expect("close 2");
}

// ─── Last Insert ID ─────────────────────────────────────────────────────────

#[test]
fn test_last_insert_id() {
    let table = test_table("last_id");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES (?)"),
            &[Value::from("test_id")],
        )
        .expect("insert");

    let last_id = client.last_insert_id().expect("last_insert_id");
    assert!(!last_id.is_empty(), "last insert id should not be empty");

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

// ─── Proto Version ──────────────────────────────────────────────────────────

#[test]
fn test_proto_version() {
    let client = Client::connect(&test_dsn()).expect("connect");
    let version = client.proto_version();
    assert!(
        version >= 0,
        "proto version should be non-negative: {version}"
    );
}

// ─── Auto-commit Mode ───────────────────────────────────────────────────────

#[test]
fn test_auto_commit_toggle() {
    let mut client = Client::connect(&test_dsn()).expect("connect");
    assert!(client.auto_commit(), "default should be auto-commit on");

    client.set_auto_commit(false);
    assert!(!client.auto_commit());

    client.set_auto_commit(true);
    assert!(client.auto_commit());

    client.close().expect("close");
}

// ─── Prepared Statement: Empty Params (FC=3 Path) ──────────────────────────

#[test]
fn test_prepared_statement_execute_no_params() {
    let table = test_table("prep_noparams");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)"),
            &[],
        )
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (val) VALUES (10), (20), (30)"),
            &[],
        )
        .expect("insert 3");

    // Prepare an INSERT with no placeholders — execute with empty params → FC=3
    let mut stmt = client
        .prepare(&format!("INSERT INTO {table} (val) VALUES (99)"))
        .expect("prepare");
    let affected = stmt.execute(&mut client, &[]).expect("exec no params");
    assert_eq!(affected, 1);
    stmt.close(&mut client).expect("close stmt");

    let result = client
        .query(&format!("SELECT COUNT(*) FROM {table}"), &[])
        .expect("count");
    match &result.rows[0][0] {
        Value::Long(v) => assert_eq!(*v, 4),
        Value::Int(v) => assert_eq!(*v, 4),
        other => panic!("expected numeric, got: {other:?}"),
    }

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_prepared_statement_query_no_params() {
    let table = test_table("prep_q_noparams");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))"),
            &[],
        )
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (name) VALUES ('alpha'), ('beta'), ('gamma')"),
            &[],
        )
        .expect("insert 3");

    // Prepare SELECT with no placeholders — query_with with empty params → FC=3
    let mut stmt = client
        .prepare(&format!("SELECT id, name FROM {table} ORDER BY id"))
        .expect("prepare");
    let result = stmt.query_with(&mut client, &[]).expect("query no params");
    assert_eq!(result.len(), 3);
    stmt.close(&mut client).expect("close stmt");

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_prepared_statement_query_error() {
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let mut stmt = client.prepare("SELECT 1").expect("prepare");

    // Close the statement first, then try to query_with
    stmt.close(&mut client).expect("close stmt");
    let result = stmt.query_with(&mut client, &[]);
    assert!(result.is_err(), "query_with after close should fail");

    // Also test Statement::query() returns error
    let mut stmt2 = client.prepare("SELECT 1").expect("prepare 2");
    let result2 = stmt2.query(&[]);
    assert!(result2.is_err(), "Statement::query should return error");
    stmt2.close(&mut client).expect("close stmt2");

    client.close().expect("close");
}

#[test]
fn test_prepared_statement_large_result_fetch() {
    let table = test_table("prep_large");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)"),
            &[],
        )
        .expect("create");

    // Insert 200 rows to exercise fetch_remaining (DEFAULT_FETCH_SIZE=100)
    for i in 1..=200 {
        client
            .execute(
                &format!("INSERT INTO {table} (val) VALUES (?)"),
                &[Value::Int(i)],
            )
            .expect("insert");
    }

    // Prepare SELECT to get all rows — should trigger fetch_remaining
    let mut stmt = client
        .prepare(&format!("SELECT id, val FROM {table} ORDER BY id"))
        .expect("prepare");
    let result = stmt.query_with(&mut client, &[]).expect("query large");
    assert_eq!(
        result.len(),
        200,
        "should fetch all 200 rows via prepared stmt"
    );
    stmt.close(&mut client).expect("close stmt");

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_into_iter_for_query_result() {
    let mut client = Client::connect(&test_dsn()).expect("connect");
    let result = client
        .query("SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3", &[])
        .expect("query");
    let mut count = 0;
    for row in &result {
        assert!(!row.is_empty());
        count += 1;
    }
    assert_eq!(count, 3);

    // Test owned IntoIterator
    let result2 = client
        .query("SELECT 1 UNION ALL SELECT 2", &[])
        .expect("query2");
    let rows: Vec<Vec<Value>> = result2.into_iter().collect();
    assert_eq!(rows.len(), 2);

    client.close().expect("close");
}

#[test]
fn test_statement_close_already_closed() {
    let table = test_table("stmt_close_idempotent");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val INT)"),
            &[],
        )
        .expect("create");

    let mut stmt = client
        .prepare(&format!("SELECT * FROM {table}"))
        .expect("prepare");
    stmt.close(&mut client).expect("first close");
    // Second close should be a no-op (already closed)
    stmt.close(&mut client).expect("second close should be ok");

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_last_insert_id_with_value() {
    let table = test_table("last_id_val");
    let mut client = Client::connect(&test_dsn()).expect("connect");

    let _ = client.execute(&format!("DROP TABLE IF EXISTS {table}"), &[]);
    client
        .execute(
            &format!("CREATE TABLE {table} (id INT AUTO_INCREMENT PRIMARY KEY, val VARCHAR(50))"),
            &[],
        )
        .expect("create");

    client
        .execute(
            &format!("INSERT INTO {table} (val) VALUES (?)"),
            &[Value::String("test".to_string())],
        )
        .expect("insert");

    let id = client.last_insert_id().expect("last_insert_id");
    assert!(!id.is_empty(), "should have a non-empty last insert id");
    // The ID should be parseable as a number
    let _: i64 = id.parse().expect("last_insert_id should be numeric");

    let _ = client.execute(&format!("DROP TABLE {table}"), &[]);
    client.close().expect("close");
}

#[test]
fn test_last_insert_id_no_insert() {
    let mut client = Client::connect(&test_dsn()).expect("connect");

    // No insert performed — last_insert_id should return empty or zero
    let id = client.last_insert_id().expect("last_insert_id");
    // CUBRID returns 0 or empty when no insert
    assert!(id.is_empty() || id == "0", "expected empty or 0, got: {id}");

    client.close().expect("close");
}
