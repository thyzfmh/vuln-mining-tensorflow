//! Compile-time configuration for FlashDB.
//!
//! Mirrors the C `fdb_cfg.h` / `fdb_cfg_template.h` configuration macros
//! as Rust constants.

/// Flash write granularity in bits.
/// Only 1 (nor flash), 8, 32, 64, 128, 256 are supported.
/// Default: 1 (nor flash).
pub const FDB_WRITE_GRAN: usize = 1;

/// Maximum KV name length.
pub const FDB_KV_NAME_MAX: usize = 64;

/// KV cache table size (improves search speed).
pub const FDB_KV_CACHE_TABLE_SIZE: usize = 64;

/// Sector cache table size (improves save speed).
pub const FDB_SECTOR_CACHE_TABLE_SIZE: usize = 8;

/// File cache table size (improves GC speed in file mode).
pub const FDB_FILE_CACHE_TABLE_SIZE: usize = 2;

/// Whether KV cache is enabled.
pub const FDB_KV_USING_CACHE: bool = FDB_KV_CACHE_TABLE_SIZE > 0 && FDB_SECTOR_CACHE_TABLE_SIZE > 0;

/// Whether file mode (POSIX or libc) is enabled.
pub const FDB_USING_FILE_MODE: bool = true;

/// Whether POSIX file mode is enabled (vs libc).
pub const FDB_USING_FILE_POSIX_MODE: bool = true;

/// Whether debug logging is enabled.
pub const FDB_DEBUG_ENABLE: bool = true;

/// Software version string.
pub const FDB_SW_VERSION: &str = "2.2.99";

/// Software version number (hex).
pub const FDB_SW_VERSION_NUM: u32 = 0x20299;
