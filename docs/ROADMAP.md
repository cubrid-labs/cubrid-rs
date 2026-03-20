> **Last updated**: 2026-03-20
>
> For the ecosystem-wide view, see the
> [CUBRID Labs Ecosystem Roadmap](https://github.com/cubrid-labs/.github/blob/main/ROADMAP.md).
>
> 📋 [GitHub Milestones](https://github.com/cubrid-labs/cubrid-rs/milestones) ·
> 🗂️ [Org Project Board](https://github.com/orgs/cubrid-labs/projects/2)

---


# Roadmap

## v0.1.0 - Project Scaffold (Current)
- [x] Multi-crate Cargo workspace
- [x] Project infrastructure (CI, labels, templates)
- [x] Documentation (PRD, TDD, Architecture, Protocol Research)

## v0.2.0 - Protocol Foundation
- [ ] Packet framing (read/write)
- [ ] Broker handshake
- [ ] Database authentication
- [ ] Connection close

## v0.3.0 - Minimal Sync Driver
- [ ] `cubrid-client::Client::connect()`
- [ ] Simple query execution
- [ ] Result row iteration
- [ ] Type mapping (basic types)

## v0.4.0 - Prepared Statements
- [ ] Bind parameter encoding
- [ ] Server-side cursors
- [ ] Lazy fetch (batches of 100)

## v0.5.0 - Async Driver
- [ ] `cubrid-tokio::Client`
- [ ] Async connect, query, fetch
- [ ] Backpressure handling

## v0.6.0 - Connection Pool
- [ ] `cubrid-pool::Pool`
- [ ] Health checks
- [ ] Idle connection management

## v1.0.0 - Stable Release
- [ ] API freeze
- [ ] Full type support
- [ ] Performance benchmarks
- [ ] Published to crates.io
