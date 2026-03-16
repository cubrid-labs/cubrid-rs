//! Protocol constants — function codes, data types, sizes, magic values.

/// Broker handshake magic bytes: ASCII "CUBRK".
pub const BROKER_MAGIC: &[u8; 5] = b"CUBRK";

/// Client type identifier sent during broker handshake (JDBC-compatible).
pub const CLIENT_TYPE_JDBC: u8 = 3;

/// CAS protocol indicator bit.
pub const CAS_PROTO_INDICATOR: u8 = 0x40;

/// CAS protocol version number.
pub const CAS_PROTOCOL_VERSION: u8 = 7;

/// Combined CAS version byte: `CAS_PROTO_INDICATOR | CAS_PROTOCOL_VERSION`.
pub const CAS_VERSION: u8 = CAS_PROTO_INDICATOR | CAS_PROTOCOL_VERSION;

/// Size of the CAS info header in framed packets (4 bytes).
pub const SIZE_CAS_INFO: usize = 4;

/// Size of the data length prefix in framed packets (4 bytes).
pub const SIZE_DATA_LENGTH: usize = 4;

/// Size of broker info in open-database response (16 bytes).
pub const SIZE_BROKER_INFO: usize = 16;

/// Total size of the database open request payload (628 bytes).
pub const DB_OPEN_PAYLOAD_SIZE: usize = 628;

/// Size of the client info exchange handshake (10 bytes).
pub const CLIENT_INFO_EXCHANGE_SIZE: usize = 10;

/// Size of the broker handshake response (4 bytes — redirect port).
pub const BROKER_RESPONSE_SIZE: usize = 4;

/// Fixed-length field sizes in the open-database request.
pub const DB_NAME_SIZE: usize = 32;
/// User name field size.
pub const DB_USER_SIZE: usize = 32;
/// Password field size.
pub const DB_PASSWORD_SIZE: usize = 32;
/// Extended info filler size.
pub const DB_EXTENDED_SIZE: usize = 512;
/// Reserved filler size.
pub const DB_RESERVED_SIZE: usize = 20;

/// Default fetch size for server-side cursors.
pub const DEFAULT_FETCH_SIZE: i32 = 100;

/// OID size in bytes.
pub const SIZE_OID: usize = 8;

// Scalar wire sizes
/// Size of a byte on the wire.
pub const SIZE_BYTE: usize = 1;
/// Size of a short (i16) on the wire.
pub const SIZE_SHORT: usize = 2;
/// Size of an int (i32) on the wire.
pub const SIZE_INT: usize = 4;
/// Size of a long (i64) on the wire.
pub const SIZE_LONG: usize = 8;
/// Size of a float (f32) on the wire.
pub const SIZE_FLOAT: usize = 4;
/// Size of a double (f64) on the wire.
pub const SIZE_DOUBLE: usize = 8;
/// Size of a datetime on the wire (7 × i16 = 14 bytes).
pub const SIZE_DATETIME: usize = 14;

/// CAS function codes for request packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FunctionCode {
    /// Commit or rollback a transaction.
    EndTran = 1,
    /// Prepare a SQL statement.
    Prepare = 2,
    /// Execute a prepared statement.
    Execute = 3,
    /// Get a database parameter.
    GetDbParameter = 4,
    /// Set a database parameter.
    SetDbParameter = 5,
    /// Close a request handle.
    CloseReqHandle = 6,
    /// Fetch rows from a server-side cursor.
    Fetch = 8,
    /// Retrieve schema information.
    SchemaInfo = 9,
    /// Get the database version string.
    GetDbVersion = 15,
    /// Execute a batch of statements.
    ExecuteBatch = 20,
    /// Close the connection.
    ConClose = 31,
    /// Get the last insert ID.
    GetLastInsertId = 40,
    /// Prepare and execute in a single round trip.
    PrepareAndExecute = 41,
}

/// CUBRID column data types as defined by the CAS protocol.
///
/// These values match the wire format exactly — the discriminant is the
/// byte sent/received on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DataType {
    /// NULL value.
    Null = 0,
    /// Fixed-length character string.
    Char = 1,
    /// Variable-length string (VARCHAR).
    String = 2,
    /// National character string.
    NChar = 3,
    /// Variable-length national character string.
    VarNChar = 4,
    /// Fixed-length bit string.
    Bit = 5,
    /// Variable-length bit string.
    VarBit = 6,
    /// Arbitrary precision numeric.
    Numeric = 7,
    /// 32-bit signed integer.
    Int = 8,
    /// 16-bit signed integer.
    Short = 9,
    /// Monetary value.
    Monetary = 10,
    /// 32-bit IEEE float.
    Float = 11,
    /// 64-bit IEEE double.
    Double = 12,
    /// Date (year, month, day).
    Date = 13,
    /// Time (hour, minute, second).
    Time = 14,
    /// Timestamp (date + time, no ms).
    Timestamp = 15,
    /// Collection: SET.
    Set = 16,
    /// Collection: MULTISET.
    Multiset = 17,
    /// Collection: SEQUENCE / LIST.
    Sequence = 18,
    /// Object reference.
    Object = 19,
    /// 64-bit signed integer.
    Bigint = 21,
    /// Datetime (date + time with milliseconds).
    Datetime = 22,
    /// Binary large object.
    Blob = 23,
    /// Character large object.
    Clob = 24,
    /// Enumeration.
    Enum = 25,
}

impl DataType {
    /// Try to convert a raw byte to a `DataType`.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Null),
            1 => Some(Self::Char),
            2 => Some(Self::String),
            3 => Some(Self::NChar),
            4 => Some(Self::VarNChar),
            5 => Some(Self::Bit),
            6 => Some(Self::VarBit),
            7 => Some(Self::Numeric),
            8 => Some(Self::Int),
            9 => Some(Self::Short),
            10 => Some(Self::Monetary),
            11 => Some(Self::Float),
            12 => Some(Self::Double),
            13 => Some(Self::Date),
            14 => Some(Self::Time),
            15 => Some(Self::Timestamp),
            16 => Some(Self::Set),
            17 => Some(Self::Multiset),
            18 => Some(Self::Sequence),
            19 => Some(Self::Object),
            21 => Some(Self::Bigint),
            22 => Some(Self::Datetime),
            23 => Some(Self::Blob),
            24 => Some(Self::Clob),
            25 => Some(Self::Enum),
            _ => None,
        }
    }

    /// Get the database type name as a string.
    pub fn type_name(self) -> &'static str {
        match self {
            Self::Null => "NULL",
            Self::Char => "CHAR",
            Self::String => "VARCHAR",
            Self::NChar => "NCHAR",
            Self::VarNChar => "VARNCHAR",
            Self::Bit => "BIT",
            Self::VarBit => "VARBIT",
            Self::Numeric => "NUMERIC",
            Self::Int => "INTEGER",
            Self::Short => "SMALLINT",
            Self::Monetary => "MONETARY",
            Self::Float => "FLOAT",
            Self::Double => "DOUBLE",
            Self::Date => "DATE",
            Self::Time => "TIME",
            Self::Timestamp => "TIMESTAMP",
            Self::Set => "SET",
            Self::Multiset => "MULTISET",
            Self::Sequence => "SEQUENCE",
            Self::Object => "OBJECT",
            Self::Bigint => "BIGINT",
            Self::Datetime => "DATETIME",
            Self::Blob => "BLOB",
            Self::Clob => "CLOB",
            Self::Enum => "ENUM",
        }
    }
}

/// Statement types returned by the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StatementType {
    /// ALTER CLASS statement.
    AlterClass = 0,
    /// ALTER SERIAL statement.
    AlterSerial = 1,
    /// COMMIT WORK statement.
    CommitWork = 2,
    /// REGISTER DATABASE statement.
    RegisterDb = 3,
    /// CREATE CLASS statement.
    CreateClass = 4,
    /// CREATE INDEX statement.
    CreateIndex = 5,
    /// CREATE TRIGGER statement.
    CreateTrigger = 6,
    /// CREATE SERIAL statement.
    CreateSerial = 7,
    /// DROP DATABASE statement.
    DropDatabase = 8,
    /// DROP CLASS statement.
    DropClass = 9,
    /// DROP INDEX statement.
    DropIndex = 10,
    /// DROP LABEL statement.
    DropLabel = 11,
    /// DROP TRIGGER statement.
    DropTrigger = 12,
    /// DROP SERIAL statement.
    DropSerial = 13,
    /// EVALUATE statement.
    Evaluate = 14,
    /// RENAME CLASS statement.
    RenameClass = 15,
    /// ROLLBACK WORK statement.
    RollbackWork = 16,
    /// GRANT statement.
    Grant = 17,
    /// REVOKE statement.
    Revoke = 18,
    /// STATISTICS statement.
    Statistics = 19,
    /// INSERT statement.
    Insert = 20,
    /// SELECT statement.
    Select = 21,
    /// UPDATE statement.
    Update = 22,
    /// DELETE statement.
    Delete = 23,
    /// CALL statement.
    Call = 24,
    /// Stored procedure call.
    CallSP = 0x7E,
    /// Unknown statement type.
    Unknown = 0x7F,
}

impl StatementType {
    /// Try to convert a raw byte to a `StatementType`.
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::AlterClass,
            1 => Self::AlterSerial,
            2 => Self::CommitWork,
            3 => Self::RegisterDb,
            4 => Self::CreateClass,
            5 => Self::CreateIndex,
            6 => Self::CreateTrigger,
            7 => Self::CreateSerial,
            8 => Self::DropDatabase,
            9 => Self::DropClass,
            10 => Self::DropIndex,
            11 => Self::DropLabel,
            12 => Self::DropTrigger,
            13 => Self::DropSerial,
            14 => Self::Evaluate,
            15 => Self::RenameClass,
            16 => Self::RollbackWork,
            17 => Self::Grant,
            18 => Self::Revoke,
            19 => Self::Statistics,
            20 => Self::Insert,
            21 => Self::Select,
            22 => Self::Update,
            23 => Self::Delete,
            24 => Self::Call,
            0x7E => Self::CallSP,
            _ => Self::Unknown,
        }
    }

    /// Check if this is a SELECT-type statement.
    pub fn is_select(self) -> bool {
        self == Self::Select
    }

    /// Check if this is a stored procedure call.
    pub fn is_call(self) -> bool {
        matches!(self, Self::Call | Self::CallSP)
    }
}

/// Transaction type for EndTran requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TransactionType {
    /// Commit the transaction.
    Commit = 1,
    /// Rollback the transaction.
    Rollback = 2,
}

/// Prepare flags.
pub const PREPARE_NORMAL: u8 = 0x00;
/// Prepare with holdable cursor.
pub const PREPARE_HOLDABLE: u8 = 0x08;

/// Execute flags.
pub const EXECUTE_NORMAL: u8 = 0x00;
/// Execute and query all results.
pub const EXECUTE_QUERY_ALL: u8 = 0x02;

#[cfg(test)]
mod tests {
    use super::*;

    // ─── DataType::from_u8 ───────────────────────────────────────────────────

    #[test]
    fn test_data_type_from_u8_all_variants() {
        let cases: &[(u8, DataType)] = &[
            (0, DataType::Null),
            (1, DataType::Char),
            (2, DataType::String),
            (3, DataType::NChar),
            (4, DataType::VarNChar),
            (5, DataType::Bit),
            (6, DataType::VarBit),
            (7, DataType::Numeric),
            (8, DataType::Int),
            (9, DataType::Short),
            (10, DataType::Monetary),
            (11, DataType::Float),
            (12, DataType::Double),
            (13, DataType::Date),
            (14, DataType::Time),
            (15, DataType::Timestamp),
            (16, DataType::Set),
            (17, DataType::Multiset),
            (18, DataType::Sequence),
            (19, DataType::Object),
            (21, DataType::Bigint),
            (22, DataType::Datetime),
            (23, DataType::Blob),
            (24, DataType::Clob),
            (25, DataType::Enum),
        ];
        for &(byte, expected) in cases {
            assert_eq!(
                DataType::from_u8(byte),
                Some(expected),
                "from_u8({byte}) should be {expected:?}"
            );
        }
    }

    #[test]
    fn test_data_type_from_u8_invalid() {
        assert_eq!(DataType::from_u8(20), None, "20 is a gap (no Bigint=21)");
        assert_eq!(DataType::from_u8(26), None);
        assert_eq!(DataType::from_u8(255), None);
    }

    // ─── DataType::type_name ────────────────────────────────────────────────

    #[test]
    fn test_data_type_type_name_all_variants() {
        let cases: &[(DataType, &str)] = &[
            (DataType::Null, "NULL"),
            (DataType::Char, "CHAR"),
            (DataType::String, "VARCHAR"),
            (DataType::NChar, "NCHAR"),
            (DataType::VarNChar, "VARNCHAR"),
            (DataType::Bit, "BIT"),
            (DataType::VarBit, "VARBIT"),
            (DataType::Numeric, "NUMERIC"),
            (DataType::Int, "INTEGER"),
            (DataType::Short, "SMALLINT"),
            (DataType::Monetary, "MONETARY"),
            (DataType::Float, "FLOAT"),
            (DataType::Double, "DOUBLE"),
            (DataType::Date, "DATE"),
            (DataType::Time, "TIME"),
            (DataType::Timestamp, "TIMESTAMP"),
            (DataType::Set, "SET"),
            (DataType::Multiset, "MULTISET"),
            (DataType::Sequence, "SEQUENCE"),
            (DataType::Object, "OBJECT"),
            (DataType::Bigint, "BIGINT"),
            (DataType::Datetime, "DATETIME"),
            (DataType::Blob, "BLOB"),
            (DataType::Clob, "CLOB"),
            (DataType::Enum, "ENUM"),
        ];
        for &(dt, expected_name) in cases {
            assert_eq!(
                dt.type_name(),
                expected_name,
                "{dt:?}.type_name() should be {expected_name}"
            );
        }
    }

    // ─── StatementType::from_u8 ──────────────────────────────────────────────

    #[test]
    fn test_statement_type_from_u8_all_variants() {
        let cases: &[(u8, StatementType)] = &[
            (0, StatementType::AlterClass),
            (1, StatementType::AlterSerial),
            (2, StatementType::CommitWork),
            (3, StatementType::RegisterDb),
            (4, StatementType::CreateClass),
            (5, StatementType::CreateIndex),
            (6, StatementType::CreateTrigger),
            (7, StatementType::CreateSerial),
            (8, StatementType::DropDatabase),
            (9, StatementType::DropClass),
            (10, StatementType::DropIndex),
            (11, StatementType::DropLabel),
            (12, StatementType::DropTrigger),
            (13, StatementType::DropSerial),
            (14, StatementType::Evaluate),
            (15, StatementType::RenameClass),
            (16, StatementType::RollbackWork),
            (17, StatementType::Grant),
            (18, StatementType::Revoke),
            (19, StatementType::Statistics),
            (20, StatementType::Insert),
            (21, StatementType::Select),
            (22, StatementType::Update),
            (23, StatementType::Delete),
            (24, StatementType::Call),
            (0x7E, StatementType::CallSP),
        ];
        for &(byte, expected) in cases {
            assert_eq!(
                StatementType::from_u8(byte),
                expected,
                "from_u8({byte}) should be {expected:?}"
            );
        }
    }

    #[test]
    fn test_statement_type_from_u8_unknown() {
        assert_eq!(StatementType::from_u8(25), StatementType::Unknown);
        assert_eq!(StatementType::from_u8(100), StatementType::Unknown);
        assert_eq!(StatementType::from_u8(255), StatementType::Unknown);
    }

    // ─── StatementType::is_select / is_call ─────────────────────────────────

    #[test]
    fn test_statement_type_is_select() {
        assert!(StatementType::Select.is_select());
        assert!(!StatementType::Insert.is_select());
        assert!(!StatementType::Update.is_select());
        assert!(!StatementType::Delete.is_select());
        assert!(!StatementType::Call.is_select());
    }

    #[test]
    fn test_statement_type_is_call() {
        assert!(StatementType::Call.is_call());
        assert!(StatementType::CallSP.is_call());
        assert!(!StatementType::Select.is_call());
        assert!(!StatementType::Insert.is_call());
        assert!(!StatementType::Unknown.is_call());
    }

    // ─── TransactionType ────────────────────────────────────────────────────

    #[test]
    fn test_transaction_type_values() {
        assert_eq!(TransactionType::Commit as u8, 1);
        assert_eq!(TransactionType::Rollback as u8, 2);
    }

    // ─── FunctionCode ──────────────────────────────────────────────────────

    #[test]
    fn test_function_code_values() {
        assert_eq!(FunctionCode::EndTran as u8, 1);
        assert_eq!(FunctionCode::Prepare as u8, 2);
        assert_eq!(FunctionCode::Execute as u8, 3);
        assert_eq!(FunctionCode::GetDbParameter as u8, 4);
        assert_eq!(FunctionCode::SetDbParameter as u8, 5);
        assert_eq!(FunctionCode::CloseReqHandle as u8, 6);
        assert_eq!(FunctionCode::Fetch as u8, 8);
        assert_eq!(FunctionCode::SchemaInfo as u8, 9);
        assert_eq!(FunctionCode::GetDbVersion as u8, 15);
        assert_eq!(FunctionCode::ExecuteBatch as u8, 20);
        assert_eq!(FunctionCode::ConClose as u8, 31);
        assert_eq!(FunctionCode::GetLastInsertId as u8, 40);
        assert_eq!(FunctionCode::PrepareAndExecute as u8, 41);
    }

    // ─── Constants ─────────────────────────────────────────────────────────

    #[test]
    fn test_constant_values() {
        assert_eq!(BROKER_MAGIC, b"CUBRK");
        assert_eq!(CLIENT_TYPE_JDBC, 3);
        assert_eq!(CAS_PROTO_INDICATOR, 0x40);
        assert_eq!(CAS_PROTOCOL_VERSION, 7);
        assert_eq!(CAS_VERSION, 0x47);
        assert_eq!(SIZE_CAS_INFO, 4);
        assert_eq!(SIZE_DATA_LENGTH, 4);
        assert_eq!(DEFAULT_FETCH_SIZE, 100);
        assert_eq!(SIZE_OID, 8);
        assert_eq!(PREPARE_NORMAL, 0x00);
        assert_eq!(PREPARE_HOLDABLE, 0x08);
        assert_eq!(EXECUTE_NORMAL, 0x00);
        assert_eq!(EXECUTE_QUERY_ALL, 0x02);
    }
}
