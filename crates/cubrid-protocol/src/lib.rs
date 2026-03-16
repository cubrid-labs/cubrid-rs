//! CUBRID CAS wire protocol implementation.
//!
//! This crate implements the binary protocol used to communicate with the
//! CUBRID database broker (CAS - Client Application Server). It handles:
//!
//! - Packet framing (length-prefixed, big-endian)
//! - Broker handshake and database authentication
//! - Function code request/response encoding
//! - Bind parameter serialization
//! - Column metadata and row data deserialization
//! - Error code mapping
//!
//! # Protocol Overview
//!
//! The CUBRID CAS protocol is a binary, big-endian protocol over TCP:
//!
//! 1. **Broker handshake** - 10-byte magic (`CUBRK` + client type + version)
//!    -> receive 4-byte port redirect
//! 2. **Open database** - 628-byte credential payload -> session info
//! 3. **Framed requests** - `[4-byte length][4-byte CAS_INFO][payload]`

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod codec;
pub mod constants;
pub mod error;
pub mod handshake;
pub mod request;
pub mod response;
pub mod types;
pub mod value;

pub use constants::{DataType, FunctionCode, StatementType};
pub use error::ProtocolError;
