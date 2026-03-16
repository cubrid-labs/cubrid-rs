# cubrid-rs

**Native Rust database driver for CUBRID** - sync + async, pure Rust, no FFI required.

[![crates.io](https://img.shields.io/crates/v/cubrid-client.svg)](https://crates.io/crates/cubrid-client)
[![Rust 1.70+](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![CI](https://github.com/cubrid-labs/cubrid-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/cubrid-labs/cubrid-rs/actions/workflows/ci.yml)
[![license](https://img.shields.io/github/license/cubrid-labs/cubrid-rs)](https://github.com/cubrid-labs/cubrid-rs/blob/main/LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/cubrid-labs/cubrid-rs)](https://github.com/cubrid-labs/cubrid-rs)
<!-- BADGES:END -->

## Why cubrid-rs?

| | cubrid-rs | CCI (C interface) |
|:---|:---|:---|
| **FFI Required** | No - pure Rust | Yes |
| **Cross-compilation** | Standard Cargo targets | Requires C toolchain |
| **Sync + Async** | Native crates for both | Manual wrappers |
| **Connection Pooling** | Native Rust pool crate | Manual management |
| **Deployment** | Rust binary + crates only | Shared library dependency |

`cubrid-rs` speaks the CUBRID CAS protocol directly over TCP with native Rust crates designed for modern sync and async services.

## Installation

```bash
# Sync client
cargo add cubrid-client

# Async client
cargo add cubrid-tokio
```

**Requirements**: Rust 1.70+

## Quick Start - Sync Client

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

## Quick Start - Async Client (tokio)

```rust,no_run
use cubrid_tokio::Client;

#[tokio::main]
async fn main() -> Result<(), cubrid_tokio::Error> {
    let mut client = Client::connect("cubrid://dba:@localhost:33000/demodb").await?;
    let _rows = client.query("SELECT 1 + 1", &[]).await?;
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

## Supported Features

| Feature | Status | Notes |
|:---|:---|:---|
| Protocol (`cubrid-protocol`) | 🚧 Planned | Scaffold complete, implementation in progress |
| Sync Client (`cubrid-client`) | 🚧 Planned | API stubs ready |
| Async Client (`cubrid-tokio`) | 🚧 Planned | API stubs ready |
| Pool (`cubrid-pool`) | 🚧 Planned | Config and error skeleton ready |

## Type Mapping

| CUBRID | Rust |
|:---|:---|
| `SMALLINT` | `i16` |
| `INTEGER` | `i32` |
| `BIGINT` | `i64` |
| `FLOAT` | `f32` |
| `DOUBLE`, `MONETARY` | `f64` |
| `CHAR`, `VARCHAR`, `STRING` | `String` |
| `BIT`, `VARBIT`, `BLOB`, `CLOB` | `Vec<u8>` |
| `NUMERIC` | `String` (initial) |
| `DATE`, `TIME`, `DATETIME`, `TIMESTAMP` | planned Rust date/time mapping |

## Architecture

```text
cubrid-rs/
├── crates/
│   ├── cubrid-protocol
│   ├── cubrid-client
│   ├── cubrid-tokio
│   └── cubrid-pool
├── docs/
├── examples/
└── tests/
```

## Protocol Notes

The CAS connection flow follows existing CUBRID client implementations:

1. Broker handshake (`CUBRK` + client metadata)
2. Optional CAS port redirect
3. Open-database request (fixed-size credential payload)
4. Framed request/response packets (`DATA_LENGTH + CAS_INFO + payload`)

## Documentation

| Document | Description |
|:---|:---|
| [PRD](docs/PRD.md) | Product requirements and phased plan |
| [TDD](docs/TDD.md) | Technical design decisions |
| [Architecture](docs/ARCHITECTURE.md) | Workspace and dependency graph |
| [Protocol Research](docs/PROTOCOL_RESEARCH.md) | CAS protocol reference |
| [Roadmap](docs/ROADMAP.md) | Planned releases |

## FAQ

### How do I connect?

Use the DSN format: `cubrid://[user[:password]]@host[:port]/database`.

### What Rust version is required?

Rust 1.70 or later.

### Does cubrid-rs require unsafe code?

No. All crates enforce `#![deny(unsafe_code)]`.

### Is async supported?

The async crate exists (`cubrid-tokio`) and is scaffolded; protocol/runtime implementation is in progress.

### Does this use C libraries or FFI?

No. The project is pure Rust.

## Benchmark

Benchmark tracking and cross-driver comparisons are maintained in [cubrid-benchmark](https://github.com/cubrid-labs/cubrid-benchmark).

## Ecosystem

| Package | Description | Language |
|:---|:---|:---|
| [cubrid-rs](https://github.com/cubrid-labs/cubrid-rs) | Native Rust CUBRID workspace | Rust |
| [cubrid-go](https://github.com/cubrid-labs/cubrid-go) | database/sql driver + GORM dialector | Go |
| [pycubrid](https://github.com/cubrid-labs/pycubrid) | DB-API 2.0 driver | Python |
| [sqlalchemy-cubrid](https://github.com/cubrid-labs/sqlalchemy-cubrid) | SQLAlchemy dialect | Python |
| [cubrid-client](https://github.com/cubrid-labs/cubrid-client) | TypeScript CAS client | TypeScript |
| [drizzle-cubrid](https://github.com/cubrid-labs/drizzle-cubrid) | Drizzle ORM dialect | TypeScript |
| [cubrid-cookbook](https://github.com/cubrid-labs/cubrid-cookbook) | Practical examples across ecosystems | Multi |

## License

MIT
