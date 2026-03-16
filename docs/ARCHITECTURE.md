# Architecture

## Workspace Layout

```text
cubrid-rs/
├── crates/
│   ├── cubrid-protocol/    # Wire protocol (CAS) - no I/O
│   │   └── src/lib.rs      # Constants, codec, packet framing, error types
│   ├── cubrid-client/      # Sync client - std::net::TcpStream
│   │   └── src/lib.rs      # Client, Connection, Row, Statement
│   ├── cubrid-tokio/       # Async client - tokio::net::TcpStream
│   │   └── src/lib.rs      # AsyncClient, AsyncConnection
│   └── cubrid-pool/        # Connection pooling
│       └── src/lib.rs      # Pool, PoolConfig
├── examples/               # Runnable examples
├── tests/                  # Integration tests (need Docker CUBRID)
└── docs/                   # Documentation
```

## Dependency Graph

```text
cubrid-pool ──→ cubrid-client ──→ cubrid-protocol
     │
     └────────→ cubrid-tokio ───→ cubrid-protocol
```

## Design Principles

1. **No unsafe code** - `#![deny(unsafe_code)]` in all crates
2. **No FFI** - pure Rust CAS protocol over TCP
3. **Protocol-first** - `cubrid-protocol` is I/O-agnostic; clients bring their own transport
4. **Minimal dependencies** - only `thiserror`, `bytes`, `tokio`
