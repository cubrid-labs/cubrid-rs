# cubrid-rs

**Native Rust database driver for CUBRID** — built by reverse-engineering the CAS wire protocol. Pure Rust, no FFI, sync + async.

<!-- BADGES:START -->
[![crates.io](https://img.shields.io/crates/v/cubrid-client.svg)](https://crates.io/crates/cubrid-client)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![CI](https://github.com/cubrid-labs/cubrid-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/cubrid-labs/cubrid-rs/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/badge/coverage-95.11%25-brightgreen.svg)](https://github.com/cubrid-labs/cubrid-rs)
[![license](https://img.shields.io/github/license/cubrid-labs/cubrid-rs)](https://github.com/cubrid-labs/cubrid-rs/blob/main/LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/cubrid-labs/cubrid-rs)](https://github.com/cubrid-labs/cubrid-rs)
<!-- BADGES:END -->

## Built by Reverse Engineering

cubrid-rs was not built from an official protocol specification — **none exists**. Instead, the entire CAS (Common Application Server) binary wire protocol was decoded by cross-referencing three existing open-source implementations ([cubrid-go](https://github.com/cubrid-labs/cubrid-go), [cubrid-client](https://github.com/cubrid-labs/cubrid-client), [pycubrid](https://github.com/cubrid-labs/pycubrid)), running targeted experiments against live CUBRID servers, and reading server-side C source code when the clients disagreed.

The full reverse engineering story — methodology, discoveries, pitfalls, and protocol details — is documented in **[PROTOCOL_RESEARCH.md](docs/PROTOCOL_RESEARCH.md)**.

**Key discoveries include:**
- The `FC=41` (PrepareAndExecute) function code doesn't support server-side bind parameters — requiring client-side SQL interpolation
- Stored procedure results embed actual type codes inside value data (column metadata reports NULL type)
- Two distinct write families (`write_*` raw vs `add_*` length-prefixed) that cause silent data corruption when confused
- A gap in the DataType enum (no type code 20) that's intentional, not a bug

## Why cubrid-rs?

| | cubrid-rs | CCI (C interface) |
|:---|:---|:---|
| **FFI Required** | No — pure Rust | Yes |
| **Cross-compilation** | Standard Cargo targets | Requires C toolchain |
| **Sync + Async** | Native crates for both | Manual wrappers |
| **Connection Pooling** | Native Rust pool crate | Manual management |
| **Deployment** | Rust binary + crates only | Shared library dependency |
| **Test Coverage** | 95.11% (366 tests) | Varies |

`cubrid-rs` speaks the CUBRID CAS protocol directly over TCP with native Rust crates designed for modern sync and async services.

## Installation

```bash
# Sync client
cargo add cubrid-client

# Async client (tokio)
cargo add cubrid-tokio

# Connection pool
cargo add cubrid-pool
```

**Requirements**: Rust 1.75+

## Quick Start — Sync Client

```rust,no_run
use cubrid_client::Client;

fn main() -> Result<(), cubrid_client::Error> {
    let mut client = Client::connect("cubrid://dba:@localhost:33000/demodb")?;
    let rows = client.query("SELECT * FROM athlete WHERE nation_code = ?", &["KOR"])?;
    for row in rows {
        println!("{:?}", row);
    }
    Ok(())
}
```

## Quick Start — Async Client (tokio)

```rust,no_run
use cubrid_tokio::Client;

#[tokio::main]
async fn main() -> Result<(), cubrid_tokio::Error> {
    let mut client = Client::connect("cubrid://dba:@localhost:33000/demodb").await?;
    let rows = client.query("SELECT 1 + 1", &[]).await?;
    for row in rows {
        println!("{:?}", row);
    }
    Ok(())
}
```

## Quick Start — Connection Pool

```rust,no_run
use cubrid_pool::Pool;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let pool = Pool::builder()
        .max_size(10)
        .build("cubrid://dba:@localhost:33000/demodb")
        .await?;

    let mut client = pool.get().await?;
    let rows = client.query("SELECT 1", &[]).await?;
    println!("{:?}", rows);
    Ok(())
}
```

## DSN Format

```text
cubrid://[user[:password]]@host[:port]/database
```

| Parameter | Default | Description |
|:---|:---|:---|
| `host` | `localhost` | CUBRID broker host |
| `port` | `33000` | CUBRID broker port |
| `database` | *(required)* | Target database name |
| `user` | `""` | Database user |
| `password` | `""` | Database password |

## Crate Overview

| Crate | Status | Description |
|:---|:---|:---|
| [`cubrid-protocol`](https://crates.io/crates/cubrid-protocol) | ✅ Stable | CAS wire protocol codec — zero I/O dependencies |
| [`cubrid-client`](https://crates.io/crates/cubrid-client) | ✅ Stable | Synchronous client with full query/transaction support |
| [`cubrid-tokio`](https://crates.io/crates/cubrid-tokio) | ✅ Stable | Async client built on tokio |
| [`cubrid-pool`](https://crates.io/crates/cubrid-pool) | ✅ Stable | Async connection pool with configurable limits |

## Type Mapping

| CUBRID | Rust | Notes |
|:---|:---|:---|
| `SMALLINT` | `i16` | |
| `INTEGER` | `i32` | |
| `BIGINT` | `i64` | |
| `FLOAT` | `f32` | |
| `DOUBLE`, `MONETARY` | `f64` | |
| `CHAR`, `VARCHAR`, `STRING` | `String` | |
| `NCHAR`, `VARNCHAR` | `String` | |
| `BIT`, `VARBIT` | `Vec<u8>` | |
| `NUMERIC` | `String` | Preserves arbitrary precision |
| `DATE` | `String` | `"YYYY-MM-DD"` |
| `TIME` | `String` | `"HH:MM:SS"` |
| `DATETIME` | `String` | `"YYYY-MM-DD HH:MM:SS.fff"` |
| `TIMESTAMP` | `String` | `"YYYY-MM-DD HH:MM:SS"` |
| `BLOB` | `Vec<u8>` | |
| `CLOB` | `String` | |
| `SET`, `MULTISET`, `SEQUENCE` | `Vec<Value>` | Nested type-tagged arrays |
| `ENUM` | `String` | |
| `OBJECT` | `String` | OID representation |

## Architecture

```text
cubrid-rs/                          4 crates, 1 workspace
├── crates/
│   ├── cubrid-protocol/            Pure protocol codec (no I/O)
│   │   ├── constants.rs            CAS function codes, type codes, flags
│   │   ├── handshake.rs            Broker handshake + OpenDatabase
│   │   ├── codec.rs                PacketWriter / PacketReader
│   │   ├── request.rs              Request frame builders
│   │   ├── response.rs             Response parsers
│   │   └── value.rs                Value enum + type conversions
│   ├── cubrid-client/              Sync TCP client
│   ├── cubrid-tokio/               Async tokio client
│   └── cubrid-pool/                Async connection pool
├── docs/
│   ├── PROTOCOL_RESEARCH.md        ★ Reverse engineering narrative
│   ├── PRD.md                      Product requirements
│   ├── TDD.md                      Technical design decisions
│   ├── ARCHITECTURE.md             Workspace + dependency graph
│   └── ROADMAP.md                  Release plan
└── examples/
```

## Protocol Notes

The CAS connection flow, decoded by reverse engineering:

1. **Broker handshake** — Send `CUBRK` + client metadata (10 bytes), receive CAS port redirect (4 bytes)
2. **Open database** — Send 628-byte fixed credential payload, receive session + protocol version
3. **Framed RPC** — All operations use `[DATA_LENGTH][CAS_INFO][FC + args]` frames
4. **Function codes** — 11 core FCs implemented: PREPARE, EXECUTE, FETCH, END_TRAN, etc.

See [PROTOCOL_RESEARCH.md](docs/PROTOCOL_RESEARCH.md) for the complete story.

## Documentation

| Document | Description |
|:---|:---|
| [Protocol Research](docs/PROTOCOL_RESEARCH.md) | **★** Full reverse engineering narrative — methodology, discoveries, pitfalls |
| [PRD](docs/PRD.md) | Product requirements and phased plan |
| [TDD](docs/TDD.md) | Technical design decisions |
| [Architecture](docs/ARCHITECTURE.md) | Workspace and dependency graph |
| [Roadmap](docs/ROADMAP.md) | Planned releases |

## FAQ

### How do I connect?

Use the DSN format: `cubrid://[user[:password]]@host[:port]/database`.

### What Rust version is required?

Rust 1.75 or later.

### Does cubrid-rs require unsafe code?

No. All crates enforce `#![deny(unsafe_code)]`.

### Is async supported?

Yes. `cubrid-tokio` provides a fully async client built on tokio. `cubrid-pool` provides async connection pooling on top of it.

### Does this use C libraries or FFI?

No. The project is pure Rust. The CAS protocol was reverse-engineered and implemented from scratch.

### How was the protocol decoded?

By cross-referencing three existing open-source client implementations (Go, TypeScript, Python) and testing against live CUBRID servers. See [PROTOCOL_RESEARCH.md](docs/PROTOCOL_RESEARCH.md).

## Benchmark

Benchmark tracking and cross-driver comparisons are maintained in [cubrid-benchmark](https://github.com/cubrid-labs/cubrid-benchmark).

## Ecosystem

| Package | Description | Language |
|:---|:---|:---|
| [cubrid-rs](https://github.com/cubrid-labs/cubrid-rs) | Native Rust CUBRID workspace | Rust |
| [sea-orm-cubrid](https://github.com/cubrid-labs/sea-orm-cubrid) | SeaORM backend for CUBRID | Rust |
| [cubrid-go](https://github.com/cubrid-labs/cubrid-go) | database/sql driver + GORM dialector | Go |
| [gorm-cubrid](https://github.com/cubrid-labs/gorm-cubrid) | GORM dialect for CUBRID | Go |
| [pycubrid](https://github.com/cubrid-labs/pycubrid) | DB-API 2.0 driver | Python |
| [sqlalchemy-cubrid](https://github.com/cubrid-labs/sqlalchemy-cubrid) | SQLAlchemy dialect | Python |
| [cubrid-client](https://github.com/cubrid-labs/cubrid-client) | TypeScript CAS client | TypeScript |
| [drizzle-cubrid](https://github.com/cubrid-labs/drizzle-cubrid) | Drizzle ORM dialect | TypeScript |
| [cubrid-cookbook](https://github.com/cubrid-labs/cubrid-cookbook) | Practical examples across ecosystems | Multi |
| [cubrid-benchmark](https://github.com/cubrid-labs/cubrid-benchmark) | Multi-language benchmark suite | Multi |

## License

MIT
