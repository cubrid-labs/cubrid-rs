# PRD: cubrid-rs - Native Rust CUBRID Driver Workspace

## 1. Overview

**Project**: cubrid-rs  
**Current Version**: 0.1.0  
**Status**: Scaffold  
**Repository**: [github.com/cubrid-labs/cubrid-rs](https://github.com/cubrid-labs/cubrid-rs)  
**License**: MIT

### 1.1 Problem Statement

There is no first-class native Rust driver for CUBRID. Teams using Rust with CUBRID today are typically forced to:

- Integrate C libraries via FFI (higher build complexity and portability friction)
- Depend on CGO-style or language-bridge approaches outside the Rust ecosystem
- Build ad-hoc internal protocol clients without reusable public abstractions

Rust users expect a crate-native, type-safe, modern API that works with both sync and async application stacks.

### 1.2 What Will Be Built

`cubrid-rs` is a multi-crate Cargo workspace with clear responsibility boundaries:

- `cubrid-protocol`: CAS wire protocol, framing, function codes, error model
- `cubrid-client`: synchronous client over `std::net::TcpStream`
- `cubrid-tokio`: async client over `tokio::net::TcpStream`
- `cubrid-pool`: connection pooling for sync/async clients

## 2. Technical Architecture

### 2.1 Workspace Layout

```text
cubrid-rs/
├── crates/
│   ├── cubrid-protocol/
│   ├── cubrid-client/
│   ├── cubrid-tokio/
│   └── cubrid-pool/
├── docs/
├── examples/
└── tests/
```

### 2.2 Dependency Matrix

| Package | Version | Purpose |
|---|---|---|
| Rust | >= 1.70 | Language/runtime baseline |
| thiserror | 2.x | Structured library error types |
| bytes | 1.x | Binary protocol buffer handling |
| tokio | 1.x | Async runtime for `cubrid-tokio` and pooling |
| tracing | 0.1.x | Optional diagnostics instrumentation |

### 2.3 DSN Format

```text
cubrid://[user[:password]]@host[:port]/database
```

| Parameter | Default | Description |
|---|---|---|
| `host` | `localhost` | CUBRID broker host |
| `port` | `33000` | CUBRID broker port |
| `database` | required | Target database |
| `user` | empty | Database user |
| `password` | empty | Database password |

## 3. Development Phases

### Phase 1 - Protocol Research (Done)

- CAS packet framing and handshake sequence documented
- Function code mapping collected from existing production clients
- Message shape and response parsing strategy validated

### Phase 2 - Minimal Sync Driver

- Implement `cubrid-client::Client::connect()`
- Basic text query execution path
- Basic row/column decoding for common scalar types

### Phase 3 - Prepared Statement Support

- Handle lifecycle and prepare/execute split flow
- Bind parameter encoding and type-directed serialization
- Cursor-based fetch with configurable batch size

### Phase 4 - Async Driver (tokio)

- Async transport and framed I/O
- Backpressure-safe request/response loop
- Async query/fetch API parity with sync client

## 4. CAS Protocol Implementation

`cubrid-rs` follows the same connection lifecycle used by `cubrid-go` and `cubrid-client`:

1. **Broker handshake**: send broker magic (`CUBRK` + client metadata)
2. **CAS redirect**: read redirected CAS port (or continue on current socket)
3. **Open database**: send fixed-size credential payload and initialize session
4. **Framed requests**: exchange `[DATA_LENGTH][CAS_INFO][payload]` packets

This keeps wire compatibility with proven implementations while exposing Rust-native APIs.

## 5. Example-first Design Philosophy

### Why Example-first

CUBRID adoption in Rust is still emerging. Documentation must minimize time-to-first-success, especially for teams evaluating database portability.

Key principle: users should copy, run, and validate a real query in under a minute.

### Hello World - Sync Client

```rust,no_run
use cubrid_client::Client;

fn main() -> Result<(), cubrid_client::Error> {
    let mut client = Client::connect("cubrid://dba:@localhost:33000/demodb")?;
    let _rows = client.query("SELECT 1", &[])?;
    Ok(())
}
```

### Hello World - Async Client

```rust,no_run
use cubrid_tokio::Client;

#[tokio::main]
async fn main() -> Result<(), cubrid_tokio::Error> {
    let mut client = Client::connect("cubrid://dba:@localhost:33000/demodb").await?;
    let _rows = client.query("SELECT 1", &[]).await?;
    Ok(())
}
```

## 6. Ecosystem Integration

`cubrid-rs` is the Rust part of the cubrid-labs multi-language ecosystem:

- `pycubrid` (Python DB-API)
- `sqlalchemy-cubrid` (Python SQLAlchemy dialect)
- `cubrid-client` (TypeScript CAS client)
- `drizzle-cubrid` (TypeScript ORM integration)
- `cubrid-go` (Go database/sql driver)
- `cubrid-cookbook` (cross-language examples)

## 7. Architecture Decisions

### 7.1 Why Pure Rust

- Avoid FFI overhead and platform-specific C toolchains
- Preserve Rust safety guarantees and distribution simplicity
- Enable straightforward cross-compilation for CI and containers

### 7.2 Why Multi-crate Workspace

- Keep protocol logic transport-agnostic and reusable
- Isolate sync/async runtime concerns cleanly
- Allow users to depend only on the crates they need

### 7.3 Why Direct CAS Protocol

- Full control over behavior and compatibility
- No external binary driver dependency
- Faster iteration and clearer observability in Rust code
