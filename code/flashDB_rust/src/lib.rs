//! `flashdb_rust` — Rust reimplementation of FlashDB.
//!
//! An embedded flash database for IoT devices, supporting both KVDB
//! (key-value database) and TSDB (time-series database) modes.
//!
//! # Example
//!
//! ```rust
//! use flashdb_rust::flash::MemFlash;
//! use flashdb_rust::types::FlashBackend;
//!
//! let flash = MemFlash::new(4096, 4);
//! ```

pub mod config;
pub mod flash;
pub mod kvdb;
pub mod lowlevel;
pub mod tsdb;
pub mod types;
pub mod utils;

pub use config::*;
pub use flash::{MemFlash, PosixFlash};
pub use kvdb::*;
pub use tsdb::*;
pub use types::*;
pub use utils::*;
