# Technical Design Document (TDD)

## 1. Multi-crate Architecture Rationale

The workspace splits responsibilities across focused crates to improve maintainability, testability, and adoption:

- `cubrid-protocol`: binary protocol types, codecs, constants, error taxonomy
- `cubrid-client`: synchronous user-facing API and blocking transport
- `cubrid-tokio`: asynchronous user-facing API and Tokio transport
- `cubrid-pool`: shared pooling primitives for both execution models

This design avoids cyclic dependencies and keeps protocol code independent from I/O strategy.

## 2. Protocol Crate Design

`cubrid-protocol` owns packet format semantics and binary encoding/decoding contracts.

### Core types

- `PacketWriter`: appends primitive fields in big-endian order
- `PacketReader`: consumes primitive fields from framed payloads
- `ProtocolError`: maps parse, I/O, and server-side protocol failures

### Framing model

Each request/response frame is encoded as:

```text
[4-byte DATA_LENGTH][4-byte CAS_INFO][PAYLOAD]
```

`DATA_LENGTH` and integral payload fields are big-endian.

## 3. Sync Client Design (`cubrid-client`)

The sync client uses `std::net::TcpStream` with a blocking request/response loop:

1. Connect to broker endpoint from DSN
2. Perform broker handshake and optional CAS redirect
3. Send fixed-size open-database payload
4. Execute framed requests and parse framed responses

Public API starts minimal (`connect`, `query`) and expands as protocol support grows.

## 4. Async Client Design (`cubrid-tokio`)

The async client mirrors sync semantics using `tokio::net::TcpStream` and async read/write operations.

Design goals:

- API parity with sync where possible
- Non-blocking framed I/O
- Clear cancellation and timeout behavior via Tokio ecosystem patterns

## 5. Connection Pool Design (`cubrid-pool`)

Initial pool design is channel-based and runtime-safe:

- Bounded channel for available connections
- Checkout with backpressure instead of uncontrolled growth
- Health checks before handing out stale sockets
- Idle connection lifecycle management via timers

The crate keeps pool configuration explicit (`PoolConfig`) and avoids hidden global state.

## 6. Error Handling Strategy

All crates use `thiserror` and explicit typed errors.

Hierarchy:

- Protocol-level errors (`ProtocolError`) in `cubrid-protocol`
- Client-level wrappers in sync and async crates
- Pool-level wrappers and state errors in `cubrid-pool`

Library APIs avoid `anyhow` and preserve structured, matchable error variants.

## 7. Type Mapping Strategy

Planned type mapping baseline:

| CUBRID Type | Rust Type |
|---|---|
| `SMALLINT` | `i16` |
| `INTEGER` | `i32` |
| `BIGINT` | `i64` |
| `FLOAT` | `f32` |
| `DOUBLE`, `MONETARY` | `f64` |
| `CHAR`, `VARCHAR`, `STRING` | `String` |
| `BLOB`, `BIT`, `VARBIT` | `Vec<u8>` |
| `NUMERIC` | `String` (initial, lossless text form) |
| `DATE`, `TIME`, `DATETIME`, `TIMESTAMP` | Rust date/time types (final type to be selected) |

## 8. Testing Strategy

### Unit tests

- Focus on protocol codec correctness and edge cases
- Mock/fake framed packets for parser validation
- Validate DSN parsing and error mapping logic

### Integration tests

- Run against Dockerized CUBRID
- Cover connect, query, transaction, and basic type round-trips
- Marked/segregated to keep default local test loop fast

Primary validation command set:

- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
