//! Async CUBRID database client built on tokio.
//!
//! Provides a non-blocking, async TCP client for CUBRID using the CAS
//! wire protocol with tokio for I/O.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use cubrid_tokio::Client;
//! use cubrid_protocol::value::Value;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), cubrid_tokio::Error> {
//!     let mut client = Client::connect("cubrid://dba:@localhost:33000/demodb").await?;
//!
//!     // Execute a DDL/DML statement
//!     let affected = client.execute(
//!         "CREATE TABLE IF NOT EXISTS t (id INT AUTO_INCREMENT PRIMARY KEY, name VARCHAR(100))",
//!         &[],
//!     ).await?;
//!
//!     // Execute with parameters
//!     client.execute("INSERT INTO t (name) VALUES (?)", &[Value::from("hello")]).await?;
//!
//!     // Query rows
//!     let rows = client.query("SELECT id, name FROM t WHERE name = ?", &[Value::from("hello")]).await?;
//!     for row in &rows {
//!         println!("{:?}", row);
//!     }
//!
//!     // Transactions
//!     client.set_auto_commit(false);
//!     client.execute("INSERT INTO t (name) VALUES (?)", &[Value::from("tx_test")]).await?;
//!     client.commit().await?;
//!
//!     client.close().await?;
//!     Ok(())
//! }
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

mod conn;

pub use cubrid_client::Dsn;

use cubrid_protocol::constants::*;
use cubrid_protocol::handshake;
use cubrid_protocol::request;
use cubrid_protocol::response;
use cubrid_protocol::types::*;
use cubrid_protocol::value::Value;
use cubrid_protocol::ProtocolError;

use conn::AsyncConnection;
use thiserror::Error;

/// Async client errors.
#[derive(Debug, Error)]
pub enum Error {
    /// Protocol-level error.
    #[error("protocol error: {0}")]
    Protocol(#[from] ProtocolError),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid DSN format.
    #[error("invalid DSN: {0}")]
    InvalidDsn(String),

    /// Connection is closed.
    #[error("connection closed")]
    ConnectionClosed,

    /// Server returned an error.
    #[error("server error {code}: {message}")]
    ServerError {
        /// CUBRID error code.
        code: i32,
        /// Error message from server.
        message: String,
    },
}

/// An async CUBRID database client using tokio.
///
/// Connects to a CUBRID broker via TCP and communicates using the CAS
/// wire protocol. All operations are async.
///
/// # Connection Lifecycle
///
/// 1. [`Client::connect`] — parse DSN, TCP connect, broker handshake, open database
/// 2. Use [`Client::query`], [`Client::execute`] for SQL operations
/// 3. [`Client::close`] — send ConClose, drop TCP connection
pub struct Client {
    conn: Option<AsyncConnection>,
    auto_commit: bool,
    proto_version: i32,
}

impl Client {
    /// Connect to a CUBRID database asynchronously.
    ///
    /// # DSN Format
    ///
    /// ```text
    /// cubrid://[user[:password]]@host[:port]/database[?autocommit=true&timeout=30]
    /// ```
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use cubrid_tokio::Client;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), cubrid_tokio::Error> {
    /// let mut client = Client::connect("cubrid://dba:@localhost:33000/demodb").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(dsn: &str) -> Result<Self, Error> {
        let parsed = Dsn::parse(dsn).map_err(|e| Error::InvalidDsn(e.to_string()))?;
        Self::connect_with_dsn(&parsed).await
    }

    /// Connect using a pre-parsed [`Dsn`].
    pub async fn connect_with_dsn(dsn: &Dsn) -> Result<Self, Error> {
        let mut conn = AsyncConnection::connect(&dsn.host, dsn.port, dsn.timeout).await?;

        // Step 1: Broker handshake — send ClientInfoExchange (10 bytes)
        let exchange = handshake::write_client_info_exchange();
        conn.write_raw(&exchange).await?;

        // Step 2: Read 4-byte redirect port
        let port_bytes = conn.read_exact_bytes(BROKER_RESPONSE_SIZE).await?;
        let redirect_port =
            handshake::parse_client_info_exchange(port_bytes[..4].try_into().unwrap());

        if redirect_port < 0 {
            return Err(Error::ServerError {
                code: redirect_port,
                message: "broker rejected connection".to_string(),
            });
        }

        // Step 3: If redirect_port > 0, reconnect to the CAS port
        if redirect_port > 0 {
            conn = AsyncConnection::connect(&dsn.host, redirect_port as u16, dsn.timeout).await?;
        }

        // Step 4: Send OpenDatabase (628 bytes, unframed)
        let open_db = handshake::write_open_database(&dsn.database, &dsn.user, &dsn.password);
        conn.write_raw(&open_db).await?;

        // Step 5: Receive framed OpenDatabase response
        let response_data = conn.recv_framed().await?;
        let result = handshake::parse_open_database(&response_data)?;

        conn.set_cas_info(result.cas_info);

        Ok(Client {
            conn: Some(conn),
            auto_commit: dsn.auto_commit,
            proto_version: result.proto_version,
        })
    }

    /// Execute a SQL statement and return the number of affected rows.
    ///
    /// Uses PrepareAndExecute (FC=41) for a single round trip.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use cubrid_tokio::Client;
    /// # use cubrid_protocol::value::Value;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), cubrid_tokio::Error> {
    /// # let mut client = Client::connect("cubrid://dba:@localhost:33000/demodb").await?;
    /// let affected = client.execute("INSERT INTO t (name) VALUES (?)", &[Value::from("hello")]).await?;
    /// assert_eq!(affected, 1);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute(&mut self, sql: &str, params: &[Value]) -> Result<u64, Error> {
        let auto_commit = self.auto_commit;
        let proto_version = self.proto_version;
        let conn = self.conn_mut()?;

        let final_sql = interpolate_params(sql, params);
        let cas_info = conn.cas_info();
        let req = request::write_prepare_and_execute(&final_sql, auto_commit, &cas_info);
        conn.send_framed(&req).await?;
        let resp = conn.recv_framed().await?;

        let (result, new_cas_info) = response::parse_prepare_and_execute(&resp, proto_version)?;
        conn.set_cas_info(new_cas_info);

        // Close the server-side query handle immediately
        if result.query_handle > 0 {
            self.close_query_handle(result.query_handle).await;
        }

        let mut affected: u64 = 0;
        for info in &result.result_infos {
            affected += info.result_count as u64;
        }
        Ok(affected)
    }

    /// Execute a SQL query and return all rows.
    ///
    /// Uses PrepareAndExecute (FC=41) for a single round trip,
    /// then fetches remaining rows via Fetch (FC=8) if needed.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use cubrid_tokio::Client;
    /// # use cubrid_protocol::value::Value;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), cubrid_tokio::Error> {
    /// # let mut client = Client::connect("cubrid://dba:@localhost:33000/demodb").await?;
    /// let rows = client.query("SELECT * FROM t WHERE id > ?", &[Value::Int(0)]).await?;
    /// for row in &rows {
    ///     println!("{:?}", row);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn query(&mut self, sql: &str, params: &[Value]) -> Result<QueryResult, Error> {
        let auto_commit = self.auto_commit;
        let proto_version = self.proto_version;
        let conn = self.conn_mut()?;

        let final_sql = interpolate_params(sql, params);
        let cas_info = conn.cas_info();
        let req = request::write_prepare_and_execute(&final_sql, auto_commit, &cas_info);
        conn.send_framed(&req).await?;
        let resp = conn.recv_framed().await?;

        let (result, new_cas_info) = response::parse_prepare_and_execute(&resp, proto_version)?;
        conn.set_cas_info(new_cas_info);

        let query_handle = result.query_handle;
        let total_count = result.total_tuple_count;
        let columns = result.columns.clone();
        let stmt_type = result.statement_type;
        let mut all_rows = result.rows;

        // Fetch remaining rows if needed
        if stmt_type.is_select() && (all_rows.len() as i32) < total_count {
            self.fetch_remaining(
                query_handle,
                &columns,
                stmt_type,
                total_count,
                &mut all_rows,
            )
            .await?;
        }

        // Close handle
        if query_handle > 0 {
            self.close_query_handle(query_handle).await;
        }

        Ok(QueryResult {
            columns,
            rows: all_rows,
            total_count,
        })
    }

    /// Prepare a SQL statement for repeated execution.
    ///
    /// Returns a [`Statement`] that can be executed multiple times with
    /// different parameters.
    pub async fn prepare(&mut self, sql: &str) -> Result<Statement, Error> {
        let auto_commit = self.auto_commit;
        let conn = self.conn_mut()?;
        let cas_info = conn.cas_info();
        let req = request::write_prepare(sql, auto_commit, &cas_info);
        conn.send_framed(&req).await?;
        let resp = conn.recv_framed().await?;

        let (result, new_cas_info) = response::parse_prepare(&resp)?;
        conn.set_cas_info(new_cas_info);

        Ok(Statement {
            query_handle: result.query_handle,
            statement_type: result.statement_type,
            bind_count: result.bind_count,
            columns: result.columns,
            closed: false,
            sql: sql.to_string(),
        })
    }

    /// Commit the current transaction.
    pub async fn commit(&mut self) -> Result<(), Error> {
        self.end_tran(TransactionType::Commit).await
    }

    /// Rollback the current transaction.
    pub async fn rollback(&mut self) -> Result<(), Error> {
        self.end_tran(TransactionType::Rollback).await
    }

    /// Ping the server to check connectivity.
    ///
    /// Uses GetDbVersion (FC=15) as a lightweight health check.
    pub async fn ping(&mut self) -> Result<String, Error> {
        let auto_commit = self.auto_commit;
        let conn = self.conn_mut()?;
        let cas_info = conn.cas_info();
        let req = handshake::write_get_db_version(auto_commit, &cas_info);
        conn.send_framed(&req).await?;
        let resp = conn.recv_framed().await?;
        let (version, new_cas_info) = handshake::parse_get_db_version(&resp)?;
        conn.set_cas_info(new_cas_info);
        Ok(version)
    }

    /// Get the last auto-generated insert ID.
    pub async fn last_insert_id(&mut self) -> Result<String, Error> {
        let result = self.query("SELECT LAST_INSERT_ID()", &[]).await?;
        if let Some(row) = result.rows.first() {
            if let Some(value) = row.first() {
                return Ok(match value {
                    Value::Null => String::new(),
                    Value::Int(v) => v.to_string(),
                    Value::Long(v) => v.to_string(),
                    Value::Short(v) => v.to_string(),
                    Value::String(v) => v.clone(),
                    other => format!("{other:?}"),
                });
            }
        }
        Ok(String::new())
    }

    /// Set auto-commit mode.
    pub fn set_auto_commit(&mut self, auto_commit: bool) {
        self.auto_commit = auto_commit;
    }

    /// Get current auto-commit mode.
    pub fn auto_commit(&self) -> bool {
        self.auto_commit
    }

    /// Get the negotiated protocol version.
    pub fn proto_version(&self) -> i32 {
        self.proto_version
    }

    /// Close the connection gracefully.
    pub async fn close(&mut self) -> Result<(), Error> {
        if let Some(conn) = self.conn.take() {
            let cas_info = conn.cas_info();
            let req = handshake::write_con_close(&cas_info);
            let _ = conn.write_and_close(&req).await;
        }
        Ok(())
    }

    /// Check if the connection is closed.
    pub fn is_closed(&self) -> bool {
        self.conn.is_none()
    }

    // ─── Internal helpers ──────────────────────────────────────────────

    fn conn_mut(&mut self) -> Result<&mut AsyncConnection, Error> {
        self.conn.as_mut().ok_or(Error::ConnectionClosed)
    }

    async fn end_tran(&mut self, tran_type: TransactionType) -> Result<(), Error> {
        let conn = self.conn_mut()?;
        let cas_info = conn.cas_info();
        let req = handshake::write_end_tran(tran_type, &cas_info);
        conn.send_framed(&req).await?;
        let resp = conn.recv_framed().await?;
        let new_cas_info = parse_simple_response(&resp)?;
        conn.set_cas_info(new_cas_info);
        Ok(())
    }

    async fn close_query_handle(&mut self, handle: i32) {
        if let Some(conn) = self.conn.as_mut() {
            let cas_info = conn.cas_info();
            let req = request::write_close_req_handle(handle, &cas_info);
            if conn.send_framed(&req).await.is_ok() {
                if let Ok(resp) = conn.recv_framed().await {
                    let _ = parse_simple_response(&resp);
                }
            }
        }
    }

    async fn fetch_remaining(
        &mut self,
        query_handle: i32,
        columns: &[ColumnMetaData],
        stmt_type: StatementType,
        total_count: i32,
        rows: &mut Vec<Vec<Value>>,
    ) -> Result<(), Error> {
        let fetch_size = DEFAULT_FETCH_SIZE;
        let mut fetched_count = rows.len() as i32;

        while fetched_count < total_count {
            let conn = self.conn_mut()?;
            let cas_info = conn.cas_info();
            let req = request::write_fetch(query_handle, fetched_count, fetch_size, &cas_info);
            conn.send_framed(&req).await?;
            let resp = conn.recv_framed().await?;

            let (fetch_result, new_cas_info) = response::parse_fetch(&resp, columns, stmt_type)?;
            conn.set_cas_info(new_cas_info);

            if fetch_result.tuple_count == 0 {
                break;
            }
            fetched_count += fetch_result.tuple_count;
            rows.extend(fetch_result.rows);
        }
        Ok(())
    }
}

/// A prepared SQL statement (async).
///
/// Created via [`Client::prepare`]. Must be explicitly closed via [`Statement::close`]
/// to release the server-side query handle.
pub struct Statement {
    query_handle: i32,
    statement_type: StatementType,
    bind_count: i32,
    columns: Vec<ColumnMetaData>,
    closed: bool,
    sql: String,
}

impl Statement {
    /// Execute this prepared statement with the given parameters.
    ///
    /// Returns the number of affected rows.
    pub async fn execute(&mut self, client: &mut Client, params: &[Value]) -> Result<u64, Error> {
        if self.closed {
            return Err(Error::ConnectionClosed);
        }
        if !params.is_empty() {
            // FC=41 fallback: interpolate params into SQL, use PrepareAndExecute
            return client.execute(&self.sql, params).await;
        }
        let auto_commit = client.auto_commit;
        let proto_version = client.proto_version;
        let conn = client.conn_mut()?;
        let cas_info = conn.cas_info();
        let req = request::write_execute(
            self.query_handle,
            self.statement_type,
            params,
            auto_commit,
            &cas_info,
        );
        conn.send_framed(&req).await?;
        let resp = conn.recv_framed().await?;

        let (result, new_cas_info) =
            response::parse_execute(&resp, &self.columns, self.statement_type, proto_version)?;
        conn.set_cas_info(new_cas_info);

        let mut affected: u64 = 0;
        for info in &result.result_infos {
            affected += info.result_count as u64;
        }
        Ok(affected)
    }

    /// Query this prepared statement using the client connection.
    pub async fn query_with(
        &mut self,
        client: &mut Client,
        params: &[Value],
    ) -> Result<QueryResult, Error> {
        if self.closed {
            return Err(Error::ConnectionClosed);
        }
        if !params.is_empty() {
            // FC=41 fallback: interpolate params into SQL, use PrepareAndExecute
            return client.query(&self.sql, params).await;
        }
        let auto_commit = client.auto_commit;
        let proto_version = client.proto_version;
        let conn = client.conn_mut()?;
        let cas_info = conn.cas_info();
        let req = request::write_execute(
            self.query_handle,
            self.statement_type,
            params,
            auto_commit,
            &cas_info,
        );
        conn.send_framed(&req).await?;
        let resp = conn.recv_framed().await?;

        let (result, new_cas_info) =
            response::parse_execute(&resp, &self.columns, self.statement_type, proto_version)?;
        conn.set_cas_info(new_cas_info);

        let total_count = result.total_tuple_count;
        let mut all_rows = result.rows;

        // Fetch remaining rows
        if self.statement_type.is_select() && (all_rows.len() as i32) < total_count {
            client
                .fetch_remaining(
                    self.query_handle,
                    &self.columns,
                    self.statement_type,
                    total_count,
                    &mut all_rows,
                )
                .await?;
        }

        Ok(QueryResult {
            columns: self.columns.clone(),
            rows: all_rows,
            total_count,
        })
    }

    /// Get the number of bind parameters.
    pub fn bind_count(&self) -> i32 {
        self.bind_count
    }

    /// Get the statement type.
    pub fn statement_type(&self) -> StatementType {
        self.statement_type
    }

    /// Get column metadata.
    pub fn columns(&self) -> &[ColumnMetaData] {
        &self.columns
    }

    /// Close the prepared statement, releasing the server-side handle.
    pub async fn close(&mut self, client: &mut Client) -> Result<(), Error> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        if self.query_handle > 0 {
            client.close_query_handle(self.query_handle).await;
            self.query_handle = 0;
        }
        Ok(())
    }
}

/// Result of an async query operation.
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// Column metadata.
    pub columns: Vec<ColumnMetaData>,
    /// All rows. Each row is a `Vec<Value>` matching the column order.
    pub rows: Vec<Vec<Value>>,
    /// Total number of matching rows (server-reported).
    pub total_count: i32,
}

impl QueryResult {
    /// Get column names.
    pub fn column_names(&self) -> Vec<&str> {
        self.columns.iter().map(|c| c.name.as_str()).collect()
    }

    /// Number of rows returned.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Check if result is empty.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl<'a> IntoIterator for &'a QueryResult {
    type Item = &'a Vec<Value>;
    type IntoIter = std::slice::Iter<'a, Vec<Value>>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.iter()
    }
}

impl IntoIterator for QueryResult {
    type Item = Vec<Value>;
    type IntoIter = std::vec::IntoIter<Vec<Value>>;

    fn into_iter(self) -> Self::IntoIter {
        self.rows.into_iter()
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Parse a simple response that only contains CAS_INFO + response_code.
fn parse_simple_response(data: &[u8]) -> Result<[u8; SIZE_CAS_INFO], Error> {
    use cubrid_protocol::codec::PacketReader;

    let mut reader = PacketReader::new(data);
    let cas_info = reader.parse_cas_info()?;
    let code = reader.parse_int()?;
    if code < 0 {
        let (err_code, message) = reader.read_error(reader.remaining())?;
        return Err(Error::Protocol(ProtocolError::ServerError {
            code: err_code,
            message,
        }));
    }
    Ok(cas_info)
}

/// Interpolate bind parameters into the SQL string using client-side substitution.
fn interpolate_params(sql: &str, params: &[Value]) -> String {
    if params.is_empty() {
        return sql.to_string();
    }

    let mut result = String::with_capacity(sql.len() + params.len() * 16);
    let mut param_idx = 0;
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '?' && param_idx < params.len() {
            result.push_str(&format_value(&params[param_idx]));
            param_idx += 1;
        } else if ch == '\'' {
            // Skip through string literals without replacing ? inside them
            result.push(ch);
            for inner in chars.by_ref() {
                result.push(inner);
                if inner == '\'' {
                    break;
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Format a Value as a SQL literal.
fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Bool(v) => if *v { "1" } else { "0" }.to_string(),
        Value::Short(v) => v.to_string(),
        Value::Int(v) => v.to_string(),
        Value::Long(v) => v.to_string(),
        Value::Float(v) => v.to_string(),
        Value::Double(v) => v.to_string(),
        Value::String(v) => {
            let escaped = v.replace('\'', "''");
            format!("'{escaped}'")
        }
        Value::Bytes(v) => {
            let hex: String = v.iter().map(|b| format!("{b:02x}")).collect();
            format!("X'{hex}'")
        }
        Value::Date { year, month, day } => {
            format!("DATE'{year:04}-{month:02}-{day:02}'")
        }
        Value::Time {
            hour,
            minute,
            second,
        } => {
            format!("TIME'{hour:02}:{minute:02}:{second:02}'")
        }
        Value::Timestamp {
            year,
            month,
            day,
            hour,
            minute,
            second,
        } => {
            format!("TIMESTAMP'{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}'")
        }
        Value::Datetime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            ms,
        } => {
            format!(
                "DATETIME'{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{ms:03}'"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_no_params() {
        let sql = "SELECT * FROM t";
        assert_eq!(interpolate_params(sql, &[]), sql);
    }

    #[test]
    fn test_interpolate_int_param() {
        let sql = "SELECT * FROM t WHERE id = ?";
        let result = interpolate_params(sql, &[Value::Int(42)]);
        assert_eq!(result, "SELECT * FROM t WHERE id = 42");
    }

    #[test]
    fn test_interpolate_string_param() {
        let sql = "INSERT INTO t (name) VALUES (?)";
        let result = interpolate_params(sql, &[Value::from("hello")]);
        assert_eq!(result, "INSERT INTO t (name) VALUES ('hello')");
    }

    #[test]
    fn test_interpolate_string_with_quotes() {
        let sql = "INSERT INTO t (name) VALUES (?)";
        let result = interpolate_params(sql, &[Value::from("it's")]);
        assert_eq!(result, "INSERT INTO t (name) VALUES ('it''s')");
    }

    #[test]
    fn test_interpolate_null() {
        let sql = "INSERT INTO t (name) VALUES (?)";
        let result = interpolate_params(sql, &[Value::Null]);
        assert_eq!(result, "INSERT INTO t (name) VALUES (NULL)");
    }

    #[test]
    fn test_interpolate_multiple_params() {
        let sql = "INSERT INTO t (a, b, c) VALUES (?, ?, ?)";
        let result = interpolate_params(
            sql,
            &[
                Value::Int(1),
                Value::from("two"),
                Value::Double(std::f64::consts::PI),
            ],
        );
        assert_eq!(
            result,
            format!(
                "INSERT INTO t (a, b, c) VALUES (1, 'two', {})",
                std::f64::consts::PI
            )
        );
    }

    #[test]
    fn test_interpolate_preserves_quoted_question_mark() {
        let sql = "SELECT * FROM t WHERE name = '?' AND id = ?";
        let result = interpolate_params(sql, &[Value::Int(1)]);
        assert_eq!(result, "SELECT * FROM t WHERE name = '?' AND id = 1");
    }

    #[test]
    fn test_format_value_bool() {
        assert_eq!(format_value(&Value::Bool(true)), "1");
        assert_eq!(format_value(&Value::Bool(false)), "0");
    }

    #[test]
    fn test_format_value_bytes() {
        let result = format_value(&Value::Bytes(vec![0xCA, 0xFE]));
        assert_eq!(result, "X'cafe'");
    }

    #[test]
    fn test_format_value_date() {
        let result = format_value(&Value::Date {
            year: 2024,
            month: 6,
            day: 15,
        });
        assert_eq!(result, "DATE'2024-06-15'");
    }

    #[test]
    fn test_format_value_time() {
        let result = format_value(&Value::Time {
            hour: 14,
            minute: 30,
            second: 45,
        });
        assert_eq!(result, "TIME'14:30:45'");
    }

    #[test]
    fn test_format_value_timestamp() {
        let result = format_value(&Value::Timestamp {
            year: 2024,
            month: 6,
            day: 15,
            hour: 14,
            minute: 30,
            second: 45,
        });
        assert_eq!(result, "TIMESTAMP'2024-06-15 14:30:45'");
    }

    #[test]
    fn test_format_value_datetime() {
        let result = format_value(&Value::Datetime {
            year: 2024,
            month: 6,
            day: 15,
            hour: 14,
            minute: 30,
            second: 45,
            ms: 123,
        });
        assert_eq!(result, "DATETIME'2024-06-15 14:30:45.123'");
    }

    #[test]
    fn test_parse_simple_response_success() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]); // cas_info
        data.extend_from_slice(&0i32.to_be_bytes()); // response_code = 0 (success)
        let cas_info = parse_simple_response(&data).unwrap();
        assert_eq!(cas_info, [0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_parse_simple_response_error() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]); // cas_info
        data.extend_from_slice(&(-1i32).to_be_bytes()); // response_code = -1 (error)
        data.extend_from_slice(&(-1i32).to_be_bytes()); // indicator
        data.extend_from_slice(&(-1001i32).to_be_bytes()); // error code
        data.extend_from_slice(b"test error\0"); // error message
        let result = parse_simple_response(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_display() {
        let err = Error::InvalidDsn("bad url".to_string());
        assert_eq!(err.to_string(), "invalid DSN: bad url");

        let err = Error::ServerError {
            code: -1001,
            message: "syntax error".to_string(),
        };
        assert_eq!(err.to_string(), "server error -1001: syntax error");

        let err = Error::ConnectionClosed;
        assert_eq!(err.to_string(), "connection closed");
    }

    #[test]
    fn test_query_result_column_names() {
        let qr = QueryResult {
            columns: vec![
                ColumnMetaData {
                    column_type: DataType::Int,
                    scale: 0,
                    precision: 10,
                    name: "id".to_string(),
                    real_name: "id".to_string(),
                    table_name: "t".to_string(),
                    is_nullable: false,
                    default_value: String::new(),
                    is_auto_increment: true,
                    is_unique_key: false,
                    is_primary_key: true,
                    is_foreign_key: false,
                },
                ColumnMetaData {
                    column_type: DataType::String,
                    scale: 0,
                    precision: 100,
                    name: "name".to_string(),
                    real_name: "name".to_string(),
                    table_name: "t".to_string(),
                    is_nullable: true,
                    default_value: String::new(),
                    is_auto_increment: false,
                    is_unique_key: false,
                    is_primary_key: false,
                    is_foreign_key: false,
                },
            ],
            rows: vec![],
            total_count: 0,
        };
        assert_eq!(qr.column_names(), vec!["id", "name"]);
        assert!(qr.is_empty());
        assert_eq!(qr.len(), 0);
    }

    #[test]
    fn test_query_result_with_rows() {
        let qr = QueryResult {
            columns: vec![],
            rows: vec![
                vec![Value::Int(1), Value::from("hello")],
                vec![Value::Int(2), Value::from("world")],
            ],
            total_count: 2,
        };
        assert_eq!(qr.len(), 2);
        assert!(!qr.is_empty());
    }

    #[test]
    fn test_query_result_into_iter() {
        let qr = QueryResult {
            columns: vec![],
            rows: vec![vec![Value::Int(1)], vec![Value::Int(2)]],
            total_count: 2,
        };

        let mut count = 0;
        for row in &qr {
            assert!(!row.is_empty());
            count += 1;
        }
        assert_eq!(count, 2);

        // Test owned iteration
        let collected: Vec<Vec<Value>> = qr.into_iter().collect();
        assert_eq!(collected.len(), 2);
    }

    #[test]
    fn test_statement_accessors() {
        let stmt = Statement {
            query_handle: 1,
            statement_type: StatementType::Select,
            bind_count: 3,
            columns: vec![],
            closed: false,
            sql: String::new(),
        };
        assert_eq!(stmt.bind_count(), 3);
        assert_eq!(stmt.statement_type(), StatementType::Select);
        assert!(stmt.columns().is_empty());
    }
}
