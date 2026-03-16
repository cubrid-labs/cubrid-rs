//! Result types returned by response parsers.
//!
//! These structs correspond to the parsed server responses for various
//! CAS function codes.

use crate::constants::{DataType, StatementType};
use crate::value::Value;

/// Metadata for a single result-set column.
///
/// Parsed from the column metadata section of `PrepareAndExecute` or
/// `Prepare` responses.
#[derive(Debug, Clone)]
pub struct ColumnMetaData {
    /// The CUBRID data type of this column.
    pub column_type: DataType,
    /// Numeric scale (for NUMERIC/DECIMAL types).
    pub scale: i16,
    /// Precision (max digits or character length).
    pub precision: i32,
    /// Column alias or label.
    pub name: String,
    /// The actual column name in the schema.
    pub real_name: String,
    /// Table name the column belongs to.
    pub table_name: String,
    /// Whether the column allows NULL values.
    pub is_nullable: bool,
    /// Default value expression (empty if none).
    pub default_value: String,
    /// Whether the column has AUTO_INCREMENT.
    pub is_auto_increment: bool,
    /// Whether the column has a UNIQUE constraint.
    pub is_unique_key: bool,
    /// Whether the column is part of the PRIMARY KEY.
    pub is_primary_key: bool,
    /// Whether the column has a FOREIGN KEY constraint.
    pub is_foreign_key: bool,
}

/// Per-statement result metadata (one per statement in a batch).
#[derive(Debug, Clone)]
pub struct ResultInfo {
    /// The type of statement that produced this result.
    pub stmt_type: StatementType,
    /// Number of affected rows (INSERT/UPDATE/DELETE) or -1 for errors.
    pub result_count: i32,
    /// Object identifier (8 bytes, opaque).
    pub oid: Vec<u8>,
}

/// Result of a `PrepareAndExecute` (FC=41) response.
#[derive(Debug, Clone)]
pub struct PrepareAndExecuteResult {
    /// Server-assigned query handle for subsequent operations.
    pub query_handle: i32,
    /// The type of the executed statement (SELECT, INSERT, etc.).
    pub statement_type: StatementType,
    /// Number of bind-parameter placeholders (`?`).
    pub bind_count: i32,
    /// Column metadata for result set columns.
    pub columns: Vec<ColumnMetaData>,
    /// Total number of matching rows (for SELECT).
    pub total_tuple_count: i32,
    /// Per-statement result info entries.
    pub result_infos: Vec<ResultInfo>,
    /// Number of rows returned inline with this response.
    pub tuple_count: i32,
    /// Inline row data (may be empty if no rows fetched yet).
    pub rows: Vec<Vec<Value>>,
}

/// Result of a `Prepare` (FC=2) response.
#[derive(Debug, Clone)]
pub struct PrepareResult {
    /// Server-assigned query handle.
    pub query_handle: i32,
    /// The type of the prepared statement.
    pub statement_type: StatementType,
    /// Number of bind-parameter placeholders.
    pub bind_count: i32,
    /// Column metadata for the result set.
    pub columns: Vec<ColumnMetaData>,
}

/// Result of an `Execute` (FC=3) response.
#[derive(Debug, Clone)]
pub struct ExecuteResult {
    /// Total number of affected/matched rows.
    pub total_tuple_count: i32,
    /// Per-statement result info entries.
    pub result_infos: Vec<ResultInfo>,
    /// Number of rows returned inline.
    pub tuple_count: i32,
    /// Inline row data (for SELECT with fetch).
    pub rows: Vec<Vec<Value>>,
}

/// Result of a `Fetch` (FC=8) response.
#[derive(Debug, Clone)]
pub struct FetchResult {
    /// Number of rows in this fetch batch.
    pub tuple_count: i32,
    /// Row data for this batch.
    pub rows: Vec<Vec<Value>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_metadata_debug() {
        let col = ColumnMetaData {
            column_type: DataType::Int,
            scale: 0,
            precision: 10,
            name: "id".to_string(),
            real_name: "id".to_string(),
            table_name: "test".to_string(),
            is_nullable: false,
            default_value: String::new(),
            is_auto_increment: true,
            is_unique_key: false,
            is_primary_key: true,
            is_foreign_key: false,
        };
        let debug = format!("{col:?}");
        assert!(debug.contains("Int"));
        assert!(debug.contains("id"));
    }

    #[test]
    fn test_result_info_debug() {
        let info = ResultInfo {
            stmt_type: StatementType::Insert,
            result_count: 1,
            oid: vec![0; 8],
        };
        let debug = format!("{info:?}");
        assert!(debug.contains("Insert"));
    }

    #[test]
    fn test_prepare_and_execute_result_empty() {
        let result = PrepareAndExecuteResult {
            query_handle: 1,
            statement_type: StatementType::Select,
            bind_count: 0,
            columns: vec![],
            total_tuple_count: 0,
            result_infos: vec![],
            tuple_count: 0,
            rows: vec![],
        };
        assert_eq!(result.query_handle, 1);
        assert_eq!(result.total_tuple_count, 0);
        assert!(result.rows.is_empty());
    }

    #[test]
    fn test_prepare_result() {
        let result = PrepareResult {
            query_handle: 42,
            statement_type: StatementType::Select,
            bind_count: 2,
            columns: vec![],
        };
        assert_eq!(result.query_handle, 42);
        assert_eq!(result.bind_count, 2);
    }

    #[test]
    fn test_execute_result() {
        let result = ExecuteResult {
            total_tuple_count: 5,
            result_infos: vec![],
            tuple_count: 0,
            rows: vec![],
        };
        assert_eq!(result.total_tuple_count, 5);
    }

    #[test]
    fn test_fetch_result() {
        let result = FetchResult {
            tuple_count: 10,
            rows: vec![vec![Value::Int(1), Value::String("test".to_string())]],
        };
        assert_eq!(result.tuple_count, 10);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].len(), 2);
    }

    #[test]
    fn test_clone() {
        let col = ColumnMetaData {
            column_type: DataType::String,
            scale: 0,
            precision: 255,
            name: "name".to_string(),
            real_name: "name".to_string(),
            table_name: "t".to_string(),
            is_nullable: true,
            default_value: "''".to_string(),
            is_auto_increment: false,
            is_unique_key: false,
            is_primary_key: false,
            is_foreign_key: false,
        };
        let col2 = col.clone();
        assert_eq!(col2.name, "name");
        assert_eq!(col2.column_type, DataType::String);
    }
}
