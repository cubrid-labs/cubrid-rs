# CUBRID CAS Protocol Research

This document captures the CUBRID CAS wire protocol behavior as observed in existing production-grade implementations.

## 1. Reference Implementations

- `cubrid-go` (Go)
- `cubrid-client` (TypeScript)

These projects are treated as implementation references for packet layout and request flow.

## 2. Connection Sequence

The connection process is two-step:

1. **Broker handshake**
   - Connect to broker host/port
   - Send 10-byte handshake magic: `CUBRK` + client metadata (type/version)
   - Receive 4-byte CAS port redirect (port `0` indicates same socket path in some server cases)

2. **Open database**
   - Connect to CAS endpoint
   - Send fixed-size open payload (628 bytes)
   - Includes database name, user, password, and mode flags
   - Receive session initialization response

After this stage, framed function-code requests can be exchanged.

## 3. Packet Framing Format

All CAS requests/responses are framed as:

```text
[4-byte DATA_LENGTH][4-byte CAS_INFO][PAYLOAD]
```

- `DATA_LENGTH`: payload length metadata (big-endian integer)
- `CAS_INFO`: fixed 4-byte header
- `PAYLOAD`: function-code-specific body

Protocol encoding is big-endian for fixed-width numeric fields.

## 4. Function Codes

Commonly used function codes in baseline client flows:

| Function Code | Name | Purpose |
|---:|---|---|
| 1 | `END_TRAN` | Commit or rollback |
| 2 | `PREPARE` | Prepare statement |
| 3 | `EXECUTE` | Execute prepared statement |
| 6 | `CLOSE_REQ_HANDLE` | Close prepared handle |
| 8 | `FETCH` | Fetch additional rows |
| 9 | `SCHEMA_INFO` | Fetch schema metadata |
| 15 | `GET_DB_VERSION` | Health/version probe |
| 20 | `EXECUTE_BATCH` | Batch execution |
| 31 | `CON_CLOSE` | Close connection |
| 40 | `GET_LAST_INSERT_ID` | Retrieve last insert id |
| 41 | `PREPARE_AND_EXECUTE` | Combined path |

## 5. Bind Parameter Encoding

Bind values use type-tagged binary records. Baseline shape:

```text
[int32 value_size][u8 data_type][value bytes]
```

Key points:

- `value_size` is bytes, big-endian
- `data_type` maps to CAS type constants
- `NULL` values are represented via null-tagged records
- Text and binary values are length-prefixed

## 6. Column Metadata Fields

Result metadata packets include per-column descriptors. Typical fields include:

- Column name
- Table/class name
- Data type code
- Precision/scale
- Nullability and key-related flags

Exact field ordering and optional flag behavior must match CAS version specifics.

## 7. Response Parsing Model

Response parsing generally follows:

1. Read frame header (`DATA_LENGTH`, `CAS_INFO`)
2. Decode status/error code area
3. Decode metadata block (for query responses)
4. Decode row block or continuation cursor state

Error responses carry server error code + message and should be surfaced as typed Rust errors.

## 8. Implementation Guidance

- Keep protocol codec pure and transport-agnostic
- Enforce bounds checks and malformed input handling
- Avoid panic-based control flow in parse paths
- Keep request/response builders version-aware where behavior diverges
