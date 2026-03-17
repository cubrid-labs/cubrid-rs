# CUBRID CAS Protocol — Reverse Engineering Documentation

This document tells the story of how the CUBRID CAS (Common Application Server) binary wire protocol was reverse-engineered to build a pure-Rust database driver from scratch. No official protocol specification exists — every byte layout, handshake sequence, and edge case documented here was decoded by cross-referencing three existing open-source client implementations, writing targeted experiments against live CUBRID servers, and carefully reading server-side C source code when the clients disagreed.

**Why this matters**: cubrid-rs is the first CUBRID driver built entirely by protocol analysis. There are no C bindings, no FFI calls, no shared libraries. The TCP connection speaks raw CAS frames. This document captures not just *what* the protocol looks like, but *how* we figured it out and *where* we got stuck.

---

## Table of Contents

1. [Methodology](#1-methodology)
2. [Reference Implementations](#2-reference-implementations)
3. [Connection Sequence](#3-connection-sequence)
4. [Packet Framing](#4-packet-framing)
5. [Function Code System](#5-function-code-system)
6. [FC=41 PrepareAndExecute — The Fallback Decision](#6-fc41-prepareandexecute--the-fallback-decision)
7. [Bind Parameter Encoding](#7-bind-parameter-encoding)
8. [Response Parsing Architecture](#8-response-parsing-architecture)
9. [Column Metadata and the Legacy Type Flag](#9-column-metadata-and-the-legacy-type-flag)
10. [Type System Mapping](#10-type-system-mapping)
11. [Stored Procedure Special Handling](#11-stored-procedure-special-handling)
12. [The Two Write Families — Our Biggest Bug Source](#12-the-two-write-families--our-biggest-bug-source)
13. [Protocol Version Conditionals](#13-protocol-version-conditionals)
14. [Design Decisions](#14-design-decisions)
15. [Verification Strategy](#15-verification-strategy)
16. [Challenges and Pitfalls](#16-challenges-and-pitfalls)
17. [Appendix A — Complete Function Code Table](#appendix-a--complete-function-code-table)
18. [Appendix B — Complete DataType Enum](#appendix-b--complete-datatype-enum)

---

## 1. Methodology

### The Problem

CUBRID does not publish a wire protocol specification. The server source code is open (C/C++), but the CAS protocol is deeply embedded in the broker architecture — there is no standalone "protocol.h" that documents frame layouts. The official client libraries (JDBC, CCI, PHP, Python) all link against the CCI C library or implement the protocol independently without shared documentation.

### Our Approach

We treated three existing client implementations as our specification:

1. **cubrid-go** — A Go `database/sql` driver that speaks CAS directly over TCP
2. **cubrid-client** — A TypeScript CAS client that implements the protocol from scratch
3. **pycubrid** — A Python DB-API 2.0 driver with its own CAS implementation

The methodology was:

1. **Read all three implementations simultaneously**, function by function, comparing how each one handles the same operation (connect, prepare, execute, fetch, close).
2. **Identify consensus** — When all three implementations agree on a byte layout, that's the protocol.
3. **Resolve disagreements** — When implementations differ (this happens more than you'd expect), test against a live CUBRID 11.2 server in Docker to determine which is correct.
4. **Build incrementally** — Implement one function code at a time, test it against the live server, then move to the next.
5. **Document as we go** — Every non-obvious discovery gets captured here.

### What Made This Hard

- **No error messages for protocol violations**. If you send a malformed packet, CUBRID usually just closes the connection. No "expected 4 bytes, got 3" — just TCP RST. This makes debugging incredibly tedious.
- **Byte-level precision matters**. Off-by-one in a length prefix means the server reads the next field's data as part of the previous field. The symptoms are nonsensical — you get garbage column names, wrong row counts, or silent data corruption.
- **Version-dependent behavior**. Some response fields only exist when the protocol version exceeds a threshold. The three reference implementations don't always handle versioning consistently.

---

## 2. Reference Implementations

### cubrid-go

The most complete and well-tested reference. Its `conn.go` and `protocol.go` files contain clear, linear CAS implementations that were straightforward to follow. Go's explicit error handling made the control flow easy to trace.

**Particularly useful for**: Connection handshake byte layouts, the FC=41 fallback decision, bind parameter encoding.

**Watch out**: Uses Go-specific idioms (big-endian readers from `encoding/binary`) that don't always make the underlying byte operations obvious.

### cubrid-client (TypeScript)

The most readable implementation. TypeScript's type annotations and class structure made it easy to understand the intent behind each protocol operation. The `CASClient` class has clean method boundaries that map 1:1 to CAS function codes.

**Particularly useful for**: Response parsing structure, column metadata decoding, type conversion.

**Watch out**: Some protocol edge cases are handled differently from cubrid-go — in those cases, cubrid-go was usually correct based on our live-server testing.

### pycubrid (Python)

The newest implementation (we built it). Useful as a cross-check because we already understood the protocol well enough to implement it in Python. When our Rust implementation disagreed with pycubrid, we knew one of them had a bug.

**Particularly useful for**: Validating our understanding. If a concept was clear enough to implement in both Python and Rust, we probably got it right.

---

## 3. Connection Sequence

The CAS connection is a two-phase process: broker handshake, then database open. This is the most critical sequence to get right — if either phase fails, CUBRID gives you nothing useful to debug with.

### Phase 1: Broker Handshake

The client connects to the broker's TCP port (default 33000) and sends a 10-byte handshake:

```text
Bytes 0-4:   b"CUBRK"              — Magic identifier
Byte 5:      0x03                   — CLIENT_TYPE_JDBC
Byte 6:      0x47                   — CAS version (0x40 | 7 = version 7 with flag)
Bytes 7-9:   0x00 0x00 0x00         — Padding
```

**Key discovery**: `CLIENT_TYPE_JDBC` (0x03) is used by all non-CCI clients, not just Java. The name is misleading — it really means "client that speaks raw CAS protocol." Using `CLIENT_TYPE_CCI` (0x05) triggers different server-side behavior (different response formats, different connection lifecycle).

**CAS version byte**: The value `0x47` is `0x40 | 7`, where `0x40` is a flag indicating the client supports certain protocol extensions and `7` is the base version. We discovered this by observing that cubrid-go sends `0x47` while older clients send different values, and the server responds with version-specific fields based on this byte.

The broker responds with a **4-byte signed integer** (big-endian):

| Value | Meaning |
|:---|:---|
| Negative | Connection rejected (broker full, auth failure, etc.) |
| Zero | Reuse same TCP connection for CAS communication |
| Positive | New CAS port — reconnect to this port |

**Redirect behavior**: In our testing with CUBRID 10.2–11.4, the broker almost always returns `0` (reuse connection). Port redirects happen in clustered/HA configurations. Our implementation handles both paths, but the redirect path is rarely exercised.

### Phase 2: Open Database

After the handshake, the client sends a fixed 628-byte `OpenDatabase` payload:

```text
Bytes 0-31:     Database name (null-padded to 32 bytes)
Bytes 32-63:    Username (null-padded to 32 bytes)
Bytes 64-95:    Password (null-padded to 32 bytes)
Bytes 96-607:   Extended URL/properties (null-padded to 512 bytes)
Bytes 608-627:  Reserved (20 bytes of zeros)
```

**Key insight**: The 512-byte "extended URL" field can contain connection properties in a URL-query format (`?key=value&key=value`), but in practice most clients leave it empty. We populate it with `""` (all zeros).

The server responds with:

```text
Bytes 0-3:    CAS_INFO (4-byte opaque session token)
Bytes 4-7:    Response code (int32, negative = error)
Bytes 8-23:   Broker info (16 bytes)
                - Byte [4] of broker_info: protocol version (masked with 0x3F)
Bytes 24+:    Session ID (remaining bytes)
```

**The CAS_INFO token**: This 4-byte value is an opaque session identifier. It MUST be echoed back in every subsequent request frame. If you send wrong CAS_INFO bytes, the server silently drops your request. We spent two days debugging "server not responding" before realizing we were sending stale CAS_INFO from a previous connection.

**Protocol version extraction**: The protocol version is embedded at byte offset 4 within the 16-byte broker_info block, masked with `0x3F`. This version number controls which response fields are present in subsequent operations (see [Section 13: Protocol Version Conditionals](#13-protocol-version-conditionals)).

---

## 4. Packet Framing

Every CAS request and response after the handshake follows a consistent frame format:

```mermaid
flowchart LR
    A[DATA_LENGTH (4)] --> B[CAS_INFO (4)]
    B --> C[PAYLOAD (variable)]
```

- **DATA_LENGTH**: 4-byte big-endian unsigned integer. Length of the PAYLOAD only (does not include the 8-byte header).
- **CAS_INFO**: 4-byte opaque token received from the server during connection. Must be echoed back verbatim on every request.
- **PAYLOAD**: Function-code-specific binary data.

### Request Payload Structure

Within the payload, CAS requests follow a consistent pattern:

```text
[FC (1 byte)] [ARG_1_SIZE (4)] [ARG_1_DATA ...] [ARG_2_SIZE (4)] [ARG_2_DATA ...] ...
```

The first byte is always the **function code** (FC). After that, arguments are encoded as length-prefixed binary blobs: 4-byte big-endian size followed by that many bytes of data.

### Response Payload Structure

Response payloads begin with:

```text
[RESPONSE_LENGTH (4)] [RESPONSE_CODE (4)] [BODY ...]
```

- `RESPONSE_LENGTH`: Total remaining response size
- `RESPONSE_CODE`: Signed int32. Negative values indicate errors. For errors, the body contains an error code (int32) and error message (length-prefixed string).

---

## 5. Function Code System

The CAS protocol is RPC-style: each operation is identified by a single-byte function code (FC). The client sends an FC in the request payload, and the server responds with function-specific data.

### Core Function Codes Used by cubrid-rs

| FC | Constant | Purpose | Notes |
|---:|:---|:---|:---|
| 1 | `END_TRAN` | Commit or rollback | Arg: 0=commit, 1=rollback |
| 2 | `PREPARE` | Prepare SQL statement | Returns statement handle + metadata |
| 3 | `EXECUTE` | Execute prepared statement | Sends bind parameters, returns affected rows |
| 6 | `CLOSE_REQ_HANDLE` | Close statement handle | Frees server-side resources |
| 8 | `FETCH` | Fetch row batch | Cursor-based, returns N rows |
| 9 | `SCHEMA_INFO` | Schema metadata query | Table/column introspection |
| 15 | `GET_DB_VERSION` | Server version probe | Used for health checks and feature detection |
| 20 | `EXECUTE_BATCH` | Batch execution | Multiple statements in one round-trip |
| 31 | `CON_CLOSE` | Close connection | Graceful connection shutdown |
| 40 | `GET_LAST_INSERT_ID` | Last auto-increment value | Post-INSERT metadata |
| 41 | `PREPARE_AND_EXECUTE` | Combined prepare+execute | One round-trip, but has limitations |

### Function Codes We Chose NOT to Implement

Some function codes exist in the CAS protocol but are either deprecated, driver-internal, or unnecessary for a modern client:

- **FC=4** (`GET_DB_PARAMETER`) — Used by CCI for internal config. Not needed.
- **FC=5** (`SET_DB_PARAMETER`) — Same as above.
- **FC=7** (`CURSOR`) — Cursor positioning. We use FETCH-based iteration instead.
- **FC=10–14** — Various schema and LOB operations. Deferred to future releases.

---

## 6. FC=41 PrepareAndExecute — The Fallback Decision

This was one of the most consequential protocol discoveries in the project.

### The Promise

FC=41 (`PREPARE_AND_EXECUTE`) combines preparation and execution into a single round-trip. In theory, this is ideal for simple queries — halving the network overhead compared to separate FC=2 + FC=3 calls.

### The Problem

**FC=41 does not support server-side bind parameters.**

We discovered this by trying to send parameterized queries through FC=41 and getting garbled results. Cross-referencing with cubrid-go confirmed: when you use FC=41, bind parameters in the SQL (the `?` placeholders) are simply ignored. The server prepares and executes the literal SQL string as-is.

### The Solution: Client-Side Parameter Interpolation

All three reference implementations solve this the same way: when using FC=41, perform client-side SQL interpolation. Replace `?` placeholders with properly escaped literal values before sending the SQL string to the server.

Our implementation in `cubrid-client/src/lib.rs`:

```rust
fn interpolate_params(sql: &str, params: &[Value]) -> String {
    let mut result = String::with_capacity(sql.len() + params.len() * 16);
    let mut param_idx = 0;
    for ch in sql.chars() {
        if ch == '?' && param_idx < params.len() {
            result.push_str(&params[param_idx].to_sql_literal());
            param_idx += 1;
        } else {
            result.push(ch);
        }
    }
    result
}
```

### The Tradeoff

| Approach | Round-trips | Bind params | SQL injection risk |
|:---|:---|:---|:---|
| FC=2 + FC=3 (Prepare + Execute) | 2 | Server-side ✅ | None |
| FC=41 (PrepareAndExecute) | 1 | Client-side interpolation | Mitigated by `to_sql_literal()` |

We chose FC=41 as our default path for all queries, with careful escaping in `to_sql_literal()`. This matches the approach used by cubrid-go and pycubrid. The escaping handles:

- Single quotes in strings (`'O''Brien'`)
- NULL as literal `NULL`
- Binary data as hex literals
- Numeric values as-is (no quoting)
- Date/time values as CUBRID timestamp literals

### Why Not Always Use FC=2 + FC=3?

Performance. In benchmarks against CUBRID 11.2, FC=41 is measurably faster for simple queries because it eliminates a network round-trip. For prepared statements that are executed many times with different parameters, FC=2 + FC=3 would be more appropriate — but cubrid-rs currently optimizes for the common case of one-shot queries.

---

## 7. Bind Parameter Encoding

When using FC=3 (`EXECUTE`) with a previously prepared statement (FC=2), bind parameters are encoded as type-tagged binary records:

```text
[int32 value_size] [u8 data_type] [value_bytes ...]
```

### Encoding Details

- `value_size`: Big-endian int32. Total size of `data_type` byte + `value_bytes`.
- `data_type`: Single byte mapping to the CAS type constant (see Appendix B).
- `value_bytes`: Type-specific binary encoding.

### NULL Handling

NULL values are encoded with a special sentinel:

```text
[int32 0x00000000]    — value_size = 0 signals NULL
```

No data_type or value_bytes follow. This is consistent across all three reference implementations.

### String Encoding

Strings are encoded as:

```text
[int32 total_size] [u8 STRING_TYPE] [string_bytes ...] [0x00]
```

Note the **null terminator** — CUBRID expects C-style null-terminated strings in the wire protocol. If you omit the trailing `0x00`, the server reads past the end of your string into the next parameter. We discovered this when our second parameter was consistently getting truncated by one byte.

### Numeric Types

Integer types use fixed-width big-endian encoding: `i16` for SHORT, `i32` for INT, `i64` for BIGINT. Floating-point values use IEEE 754 big-endian encoding.

---

## 8. Response Parsing Architecture

CAS responses follow a consistent pattern but have function-specific body formats. Our parser uses a layered approach:

### Layer 1: Frame Reading

Read the 8-byte frame header (DATA_LENGTH + CAS_INFO), then read exactly DATA_LENGTH bytes of payload.

### Layer 2: Status/Error Check

The first 4 bytes of the payload are the response code. If negative, parse the error:

```text
[int32 error_code] [int32 msg_length] [error_message_bytes ...]
```

Error codes map to CUBRID-specific error constants (e.g., `-1` = general error, `-10` = invalid handle).

### Layer 3: Function-Specific Parsing

For successful responses, parsing depends on the function code:

**FC=2 (PREPARE) response:**
```text
[int32 statement_handle]
[u8 result_cache_lifetime]
[u8 statement_type]          — SELECT=1, INSERT=2, UPDATE=3, DELETE=4, etc.
[int32 num_columns]
[column_metadata ...]        — repeated num_columns times
```

**FC=3/41 (EXECUTE/PREPARE_AND_EXECUTE) response:**
```text
[int32 num_affected]         — rows affected (for DML) or -1 (for SELECT)
[includes_column_info]       — conditional, proto_version > 1
[column_metadata ...]        — if query results exist
[row_data ...]               — inline result rows (for small result sets)
```

**FC=8 (FETCH) response:**
```text
[int32 num_rows]
[row_data ...]               — each row contains values for all columns
```

### Row Data Encoding

Each row value is encoded as:

```text
[int32 value_size] [value_bytes ...]
```

A `value_size` of `-1` indicates NULL. Otherwise, `value_bytes` contains the raw value in its wire format, which must be decoded according to the column's data type.

---

## 9. Column Metadata and the Legacy Type Flag

Column metadata is one of the most complex parts of the CAS protocol. Each column descriptor contains 13+ fields:

```text
[u8 column_type]           — data type code
[i16 scale]
[i32 precision]
[int32 name_length] [name_bytes ...]
[int32 table_length] [table_bytes ...]
[int32 alias_length] [alias_bytes ...]
[u8 is_not_null]
[u8 default_value_flag]
[u8 is_auto_increment]
[u8 is_unique_key]
[u8 is_primary_key]
[u8 is_reverse_index]
[u8 is_reverse_unique]
[u8 is_foreign_key]
[u8 is_shared]
... (additional flags depending on protocol version)
```

### The 0x80 Legacy Type Flag

**This was one of the most confusing protocol details.**

The `column_type` byte uses bit 7 (`0x80`) as a flag. When set, it indicates an "extended type" — the actual type code is in a supplementary byte that follows later in the metadata. When clear, the column_type byte directly contains the type code.

```rust
let raw_type = reader.read_u8();
let (legacy_type, has_extended) = if raw_type & 0x80 != 0 {
    (raw_type & 0x7F, true)  // mask off flag, read extended type later
} else {
    (raw_type, false)         // use directly
};
```

We initially missed this flag entirely and got wrong type codes for columns with types above 127. The symptoms were subtle — queries would "work" but return strings where we expected integers, because the type code was being read as `column_type | 0x80` instead of the actual type.

---

## 10. Type System Mapping

CUBRID defines 25 data types with an intentional gap in the numbering:

| Code | Type | Rust Mapping |
|---:|:---|:---|
| 0 | `NULL` | `Value::Null` |
| 1 | `CHAR` | `String` |
| 2 | `STRING` / `VARCHAR` | `String` |
| 3 | `NCHAR` | `String` |
| 4 | `VARNCHAR` | `String` |
| 5 | `BIT` | `Vec<u8>` |
| 6 | `VARBIT` | `Vec<u8>` |
| 7 | `NUMERIC` / `DECIMAL` | `String` (preserves precision) |
| 8 | `INT` | `i32` |
| 9 | `SHORT` / `SMALLINT` | `i16` |
| 10 | `MONETARY` | `f64` |
| 11 | `FLOAT` | `f32` |
| 12 | `DOUBLE` | `f64` |
| 13 | `DATE` | `String` ("YYYY-MM-DD") |
| 14 | `TIME` | `String` ("HH:MM:SS") |
| 15 | `TIMESTAMP` | `String` ("YYYY-MM-DD HH:MM:SS") |
| 16 | `SET` | `Vec<Value>` |
| 17 | `MULTISET` | `Vec<Value>` |
| 18 | `SEQUENCE` / `LIST` | `Vec<Value>` |
| 19 | `OBJECT` | `String` (OID representation) |
| ~~20~~ | *(gap — no type 20)* | — |
| 21 | `BIGINT` | `i64` |
| 22 | `DATETIME` | `String` ("YYYY-MM-DD HH:MM:SS.fff") |
| 23 | `BLOB` | `Vec<u8>` |
| 24 | `CLOB` | `String` |
| 25 | `ENUM` | `String` |

### The DataType 20 Gap

There is no type code 20. The enum jumps from OBJECT (19) to BIGINT (21). This is not a mistake — it's historical. We handle this in our `DataType` enum by simply not defining a variant for 20. Attempting to decode type 20 produces an error.

### Date/Time Encoding

Date and time values are transmitted as structured binary fields, NOT as strings:

```text
DATE:      [i32 year] [i32 month] [i32 day]
TIME:      [i32 hour] [i32 minute] [i32 second]
TIMESTAMP: [i32 year] [i32 month] [i32 day] [i32 hour] [i32 minute] [i32 second]
DATETIME:  [i32 year] [i32 month] [i32 day] [i32 hour] [i32 minute] [i32 second] [i32 millisecond]
```

All fields are 4-byte big-endian signed integers. We convert these to formatted strings in our `Value` enum for simplicity. A future release may add `chrono::NaiveDate`/`NaiveDateTime` support.

### Collection Types (SET, MULTISET, SEQUENCE)

Collection values are encoded as nested type-tagged arrays:

```text
[u8 element_type] [i32 num_elements] [element_1 ...] [element_2 ...] ...
```

Each element is itself a type-tagged value. This means collections can theoretically contain nested collections, though CUBRID's SQL syntax doesn't easily support this.

---

## 11. Stored Procedure Special Handling

**This was the single most non-obvious protocol behavior we encountered.**

When you execute a stored procedure via `CALL`, the result set has a peculiar property: **all column type codes are NULL (0)**. The actual data type of each value is embedded as the first byte of the value data itself.

Normal query result:
```
Column metadata: type=8 (INT)
Value: [4 bytes of int32 data]
```

Stored procedure result:
```
Column metadata: type=0 (NULL — but the value IS NOT null)
Value: [1 byte type_code] [value data ...]
```

This means the response parser must detect when a result came from a stored procedure and switch to a different parsing strategy: read the type byte from the VALUE, not from the column metadata.

### How We Detected This

We first encountered this bug when stored procedure results returned garbage data. Our parser was reading the first byte of each value as data (interpreting it as part of the integer value), producing numbers that were off by powers of 256.

Cross-referencing with cubrid-go revealed the special handling: when all column types are 0, treat the first byte of each value as the type discriminator. cubrid-client's TypeScript implementation had the same logic, confirming this is intentional CUBRID behavior.

### Implementation

```rust
fn parse_value(col_type: DataType, data: &[u8]) -> Value {
    if col_type == DataType::Null && !data.is_empty() {
        // Stored procedure result: first byte is actual type
        let actual_type = DataType::from_u8(data[0]);
        return parse_typed_value(actual_type, &data[1..]);
    }
    parse_typed_value(col_type, data)
}
```

---

## 12. The Two Write Families — Our Biggest Bug Source

The CAS protocol has two distinct patterns for writing data to the wire, and confusing them was the source of our most persistent bugs:

### Raw Writes (`write_*`)

Write bytes directly to the packet buffer WITHOUT any length prefix:

```rust
writer.write_i32(42);        // writes: 00 00 00 2A
writer.write_bytes(b"hello"); // writes: 68 65 6C 6C 6F
```

### Length-Prefixed Writes (`add_*`)

Write a 4-byte length prefix followed by the data:

```rust
writer.add_i32(42);          // writes: 00 00 00 04 00 00 00 2A
                             //         ^length=4   ^value=42
writer.add_bytes(b"hello");  // writes: 00 00 00 05 68 65 6C 6C 6F
                             //         ^length=5   ^"hello"
```

### Why This Caused Bugs

The CAS protocol requires DIFFERENT write families for DIFFERENT parts of a request:

- **Function code**: raw write (1 byte, no length prefix)
- **Arguments**: length-prefixed writes (each arg has its own length prefix)
- **Certain fields within arguments**: raw writes INSIDE a length-prefixed block

Mixing these up produces packets that parse correctly in some cases but fail in others. For example, double-length-prefixing an argument makes the server interpret the first length prefix as data, which might happen to be a valid (but wrong) value.

**Debugging clue that helped**: When a query works for `SELECT 1` but fails for `SELECT * FROM large_table`, it's probably a write-family bug. The extra length prefix gets interpreted as a small integer value that coincidentally makes sense for trivial queries but produces nonsense for real data.

### Our Solution

We clearly separated the two families in `PacketWriter`:

```rust
impl PacketWriter {
    // Raw writes — no length prefix
    fn write_i8(&mut self, v: i8) { ... }
    fn write_i32(&mut self, v: i32) { ... }
    fn write_bytes(&mut self, v: &[u8]) { ... }

    // Length-prefixed writes — automatically prepends size
    fn add_i8(&mut self, v: i8) { ... }
    fn add_i32(&mut self, v: i32) { ... }
    fn add_bytes(&mut self, v: &[u8]) { ... }
    fn add_string(&mut self, v: &str) { ... }  // includes null terminator
    fn add_null(&mut self) { ... }              // writes length=0
}
```

Every request builder function explicitly documents which write family it uses for each field, and we have unit tests that verify the exact byte sequences.

---

## 13. Protocol Version Conditionals

The CAS protocol evolves across CUBRID versions. Some response fields only exist when the negotiated protocol version exceeds a threshold:

| Proto Version | Feature |
|:---|:---|
| > 1 | `includes_column_info` field in execute response |
| > 4 | `shard_id` field in execute response |

### How Version Negotiation Works

During the connection handshake, the client sends its CAS version byte (we send `0x47` = version 7). The server responds with its supported protocol version in the broker info block (byte offset 4, masked with `0x3F`).

The effective protocol version is the **minimum** of client and server versions. This version is stored on the connection and consulted during response parsing:

```rust
if self.proto_version > 1 {
    let includes_column_info = reader.read_u8();
    // ...
}
if self.proto_version > 4 {
    let shard_id = reader.read_i32();
    // ...
}
```

### Impact on Testing

This means our test suite must handle different response formats. We test against CUBRID 10.2 through 11.4, which report different protocol versions. Our mock-based unit tests use a configurable protocol version to exercise both paths.

---

## 14. Design Decisions

### Transport-Agnostic Protocol Layer

The `cubrid-protocol` crate has zero dependencies on networking or I/O. It operates purely on byte buffers (`&[u8]` and `Vec<u8>`). This design was intentional:

1. **Testability** — Protocol parsing can be tested with hard-coded byte arrays, no network needed.
2. **Sync/Async reuse** — Both `cubrid-client` (sync) and `cubrid-tokio` (async) share the same protocol crate.
3. **Fuzz-friendliness** — Protocol parsers can be fuzzed directly without setting up network connections.

### Strict Error Handling

Every parse operation that could fail returns `Result<T, Error>`. We never `unwrap()` or `panic!()` on network data. Malformed packets produce descriptive errors:

```rust
Error::Protocol("expected 4 bytes for i32, got 2".into())
Error::Protocol("unknown data type code: 20".into())
Error::Protocol("negative column count: -1".into())
```

### Value Enum Over Generic Rows

We use a `Value` enum rather than generic type parameters for query results:

```rust
pub enum Value {
    Null,
    Short(i16),
    Int(i32),
    Bigint(i64),
    Float(f32),
    Double(f64),
    Numeric(String),
    String(String),
    Bytes(Vec<u8>),
    Date(String),
    Time(String),
    Timestamp(String),
    Datetime(String),
    Set(Vec<Value>),
    Multiset(Vec<Value>),
    Sequence(Vec<Value>),
}
```

This matches the approach used by cubrid-go and provides a simple, discoverable API. Users can pattern-match on the variant to extract typed values.

---

## 15. Verification Strategy

### Unit Tests (366 tests, 95.11% coverage)

Every protocol operation has unit tests that verify exact byte sequences:

```rust
#[test]
fn test_prepare_request_bytes() {
    let mut writer = PacketWriter::new();
    build_prepare_request(&mut writer, "SELECT 1");
    assert_eq!(writer.as_bytes(), &[
        0x02,                               // FC=2 (PREPARE)
        0x00, 0x00, 0x00, 0x08,             // string length = 8
        b'S', b'E', b'L', b'E', b'C', b'T', b' ', b'1',
        // ... additional fields
    ]);
}
```

### Integration Tests (live CUBRID server)

Integration tests run against a real CUBRID 11.2 instance in Docker:

```bash
docker run -d -p 33000:33000 cubrid/cubrid:11.2
CUBRID_DSN="cubrid://dba:@localhost:33000/demodb" cargo test -p cubrid-client --test integration
```

These tests verify:
- Connection establishment and teardown
- CRUD operations (INSERT, SELECT, UPDATE, DELETE)
- Type round-tripping (write value → read back → compare)
- Transaction commit and rollback
- Stored procedure execution
- NULL handling
- Large result sets with cursor-based fetching

### Cross-Implementation Validation

We also compared our Rust output against cubrid-go's output for the same queries. By running the same SQL against the same CUBRID server from both Go and Rust, we verified that our type conversions and result parsing produce identical values.

---

## 16. Challenges and Pitfalls

### 1. CAS_INFO Must Be Echoed

The 4-byte CAS_INFO from the server response must be stored and sent back on every subsequent request. If you initialize it to zeros or reuse CAS_INFO from a different connection, the server drops the connection without error.

### 2. Null Terminators in Strings

CUBRID expects C-style null-terminated strings in several places (database name, username, SQL text). Omitting the null terminator causes the server to read past the intended boundary. Symptoms: garbled subsequent parameters, random query failures.

### 3. The 628-Byte OpenDatabase Payload

This payload MUST be exactly 628 bytes. Sending more or fewer bytes causes the server to misparse the fields. Each field is right-padded with null bytes to its fixed width.

### 4. Big-Endian Everything

All multi-byte integers in the CAS protocol are big-endian. Rust's native byte order is platform-dependent (`cfg(target_endian)`), so all integer reads/writes must explicitly use `from_be_bytes` / `to_be_bytes`.

### 5. Statement Handle Lifecycle

Statement handles from FC=2 (PREPARE) must be explicitly closed with FC=6 (CLOSE_REQ_HANDLE) when no longer needed. Failing to close handles causes server-side resource leaks. Our `Statement` type implements `Drop` to send FC=6 automatically.

### 6. Fetch Cursor State

After an FC=3 execute that returns a SELECT result, the client must issue FC=8 (FETCH) calls to retrieve rows in batches. The fetch response includes a flag indicating whether more rows are available. If the client stops fetching before exhausting the cursor, it must still close the statement handle.

### 7. Error Code Ambiguity

Some CUBRID error codes are reused across different contexts. Error code `-1` can mean "general error" from the broker or "invalid SQL" from the CAS. We disambiguate based on which phase of the connection lifecycle the error occurred in.

---

## Appendix A — Complete Function Code Table

```rust
pub const END_TRAN: u8 = 1;
pub const PREPARE: u8 = 2;
pub const EXECUTE: u8 = 3;
pub const GET_DB_PARAMETER: u8 = 4;
pub const SET_DB_PARAMETER: u8 = 5;
pub const CLOSE_REQ_HANDLE: u8 = 6;
pub const CURSOR: u8 = 7;
pub const FETCH: u8 = 8;
pub const SCHEMA_INFO: u8 = 9;
pub const OID_GET: u8 = 10;
pub const OID_SET: u8 = 11;
pub const OID_CMD: u8 = 12;
pub const COL_GET: u8 = 13;
pub const COL_SET: u8 = 14;
pub const GET_DB_VERSION: u8 = 15;
pub const GET_CLASS_NUM_OBJS: u8 = 16;
pub const OID_CMD2: u8 = 17;
pub const EXECUTE_BATCH: u8 = 20;
pub const GET_QUERY_INFO: u8 = 24;
pub const SAVEPOINT: u8 = 25;
pub const PARAMETER_INFO: u8 = 26;
pub const XA_PREPARE: u8 = 27;
pub const XA_RECOVER: u8 = 28;
pub const XA_END_TRAN: u8 = 29;
pub const CON_CLOSE: u8 = 31;
pub const CHECK_CAS: u8 = 32;
pub const MAKE_OUT_RS: u8 = 33;
pub const GET_GENERATED_KEYS: u8 = 34;
pub const LOB_NEW: u8 = 35;
pub const LOB_WRITE: u8 = 36;
pub const LOB_READ: u8 = 37;
pub const END_SESSION: u8 = 38;
pub const GET_ROW_COUNT: u8 = 39;
pub const GET_LAST_INSERT_ID: u8 = 40;
pub const PREPARE_AND_EXECUTE: u8 = 41;
pub const CAS_CHANGE_MODE: u8 = 44;
```

## Appendix B — Complete DataType Enum

```rust
pub enum DataType {
    Null      = 0,
    Char      = 1,
    String    = 2,   // VARCHAR
    NChar     = 3,
    VarNChar  = 4,
    Bit       = 5,
    VarBit    = 6,
    Numeric   = 7,
    Int       = 8,
    Short     = 9,
    Monetary  = 10,
    Float     = 11,
    Double    = 12,
    Date      = 13,
    Time      = 14,
    Timestamp = 15,
    Set       = 16,
    Multiset  = 17,
    Sequence  = 18,
    Object    = 19,
    // 20 is intentionally missing
    Bigint    = 21,
    Datetime  = 22,
    Blob      = 23,
    Clob      = 24,
    Enum      = 25,
}
```

---

*This document is maintained as part of the [cubrid-rs](https://github.com/cubrid-labs/cubrid-rs) project. It was written during initial development of v0.1.0, based on protocol analysis of cubrid-go, cubrid-client (TypeScript), and pycubrid.*
