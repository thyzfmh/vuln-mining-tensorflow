//! Core types, enums, and constants for FlashDB.
//!
//! 1:1 behavioral translation of `fdb_def.h` and `fdb_low_lvl.h` type definitions.
//! Uses Rust idioms: enums instead of int constants, Option instead of nullable pointers,
//! String/&str instead of char buffers.

use std::fmt;

use crate::config::{
    FDB_FILE_CACHE_TABLE_SIZE, FDB_KV_CACHE_TABLE_SIZE, FDB_SECTOR_CACHE_TABLE_SIZE, FDB_WRITE_GRAN,
};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error codes, matching C `fdb_err_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdbErr {
    NoErr,
    EraseErr,
    ReadErr,
    WriteErr,
    KvNameErr,
    KvNameExist,
    SavedFull,
    InitFailed,
}

impl fmt::Display for FdbErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FdbErr::NoErr => write!(f, "NoErr"),
            FdbErr::EraseErr => write!(f, "EraseErr"),
            FdbErr::ReadErr => write!(f, "ReadErr"),
            FdbErr::WriteErr => write!(f, "WriteErr"),
            FdbErr::KvNameErr => write!(f, "KvNameErr"),
            FdbErr::KvNameExist => write!(f, "KvNameExist"),
            FdbErr::SavedFull => write!(f, "SavedFull"),
            FdbErr::InitFailed => write!(f, "InitFailed"),
        }
    }
}

impl Default for FdbErr {
    fn default() -> Self {
        FdbErr::NoErr
    }
}

// ---------------------------------------------------------------------------
// Status enums
// ---------------------------------------------------------------------------

/// KV node status, matching C `fdb_kv_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KvStatus {
    #[default]
    Unused,
    PreWrite,
    Write,
    PreDelete,
    Deleted,
    ErrHdr,
}

/// Number of KV status variants (must match C `FDB_KV_STATUS_NUM`).
pub const FDB_KV_STATUS_NUM: usize = 6;

/// TSL node status, matching C `fdb_tsl_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TslStatus {
    #[default]
    Unused,
    PreWrite,
    Write,
    UserStatus1,
    Deleted,
    UserStatus2,
}

/// Number of TSL status variants (must match C `FDB_TSL_STATUS_NUM`).
pub const FDB_TSL_STATUS_NUM: usize = 6;

/// Sector store status, matching C `fdb_sector_store_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SectorStoreStatus {
    #[default]
    Unused,
    Empty,
    Using,
    Full,
}

/// Number of sector store status variants.
pub const FDB_SECTOR_STORE_STATUS_NUM: usize = 4;

/// Sector dirty status, matching C `fdb_sector_dirty_status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SectorDirtyStatus {
    #[default]
    Unused,
    False,
    True,
    GC,
}

/// Number of sector dirty status variants.
pub const FDB_SECTOR_DIRTY_STATUS_NUM: usize = 4;

// ---------------------------------------------------------------------------
// Timestamp type
// ---------------------------------------------------------------------------

/// Timestamp type. 64-bit if `FDB_USING_TIMESTAMP_64BIT` is defined, else 32-bit.
pub type FdbTime = i32;

/// Function signature for getting the current timestamp.
pub type FdbGetTime = fn() -> FdbTime;

// ---------------------------------------------------------------------------
// Database type
// ---------------------------------------------------------------------------

/// Database type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdbDbType {
    Kv,
    Ts,
}

// ---------------------------------------------------------------------------
// Control command enums (replaces C's `int cmd + void* arg` pattern)
// ---------------------------------------------------------------------------

/// KVDB control commands with their associated argument types.
pub enum KvControlArg {
    /// Set sector size (must be done before init).
    SetSecSize(u32),
    /// Get sector size.
    GetSecSize,
    /// Set file mode (must be done before init).
    SetFileMode(bool),
    /// Set database max size in file mode (must be done before init).
    SetMaxSize(u32),
    /// Set not-format mode (must be done before init).
    SetNotFormat(bool),
}

/// TSDB control commands with their associated argument types.
pub enum TslControlArg {
    /// Set sector size (must be done before init).
    SetSecSize(u32),
    /// Get sector size.
    GetSecSize,
    /// Set rollover mode (must be done after init).
    SetRollover(bool),
    /// Get rollover mode.
    GetRollover,
    /// Get last save time.
    GetLastTime,
    /// Set file mode (must be done before init).
    SetFileMode(bool),
    /// Set database max size in file mode (must be done before init).
    SetMaxSize(u32),
    /// Set not-format mode (must be done before init).
    SetNotFormat(bool),
}

// ---------------------------------------------------------------------------
// Blob
// ---------------------------------------------------------------------------

/// Blob structure for reading/writing variable-length data.
///
/// Replaces C's `fdb_blob` (which uses raw pointers + size).
/// `buf` is the user-provided buffer, `saved` tracks where data was persisted.
#[derive(Debug, Clone)]
pub struct Blob {
    /// User-provided data buffer.
    pub buf: Vec<u8>,
    /// Metadata about the persisted blob data.
    pub saved: BlobSaved,
}

/// Saved blob metadata.
#[derive(Debug, Clone, Copy, Default)]
pub struct BlobSaved {
    /// KV or TSL index address where the blob metadata is stored.
    pub meta_addr: u32,
    /// Address where the blob data is stored.
    pub addr: u32,
    /// Length of the saved blob data.
    pub len: usize,
}

impl Blob {
    /// Create a new blob from a byte slice.
    ///
    /// Mirrors C's `fdb_blob_make()`.
    pub fn make(value_buf: &[u8]) -> Self {
        Blob {
            buf: value_buf.to_vec(),
            saved: BlobSaved::default(),
        }
    }

    /// Create an empty blob with a given buffer size (for reading).
    pub fn with_capacity(size: usize) -> Self {
        Blob {
            buf: vec![0u8; size],
            saved: BlobSaved::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Default KV
// ---------------------------------------------------------------------------

/// A single default key-value node.
#[derive(Debug, Clone)]
pub struct DefaultKvNode {
    pub key: String,
    pub value: Vec<u8>,
}

/// Default key-value set for KVDB initialization.
#[derive(Debug, Clone, Default)]
pub struct DefaultKv {
    pub kvs: Vec<DefaultKvNode>,
}

// ---------------------------------------------------------------------------
// KV node
// ---------------------------------------------------------------------------

/// KV node object, matching C `struct fdb_kv`.
#[derive(Debug, Clone)]
pub struct KvNode {
    pub status: KvStatus,
    pub crc_is_ok: bool,
    pub name_len: u8,
    pub magic: u32,
    /// Node total length (header + name + value), aligned by FDB_WRITE_GRAN.
    pub len: u32,
    pub value_len: u32,
    pub name: String,
    pub addr: KvNodeAddr,
}

/// KV node addresses.
#[derive(Debug, Clone, Default)]
pub struct KvNodeAddr {
    pub start: u32,
    pub value: u32,
}

impl Default for KvNode {
    fn default() -> Self {
        KvNode {
            status: KvStatus::Unused,
            crc_is_ok: false,
            name_len: 0,
            magic: 0,
            len: 0,
            value_len: 0,
            name: String::new(),
            addr: KvNodeAddr::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// KV iterator
// ---------------------------------------------------------------------------

/// KV iterator state, matching C `struct fdb_kv_iterator`.
#[derive(Debug, Clone)]
pub struct KvIterator {
    pub curr_kv: KvNode,
    pub iterated_cnt: u32,
    pub iterated_obj_bytes: usize,
    pub iterated_value_bytes: usize,
    pub sector_addr: u32,
    pub traversed_len: u32,
}

impl Default for KvIterator {
    fn default() -> Self {
        KvIterator {
            curr_kv: KvNode::default(),
            iterated_cnt: 0,
            iterated_obj_bytes: 0,
            iterated_value_bytes: 0,
            sector_addr: 0,
            traversed_len: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// TSL node
// ---------------------------------------------------------------------------

/// Time-series log node object, matching C `struct fdb_tsl`.
#[derive(Debug, Clone)]
pub struct TslNode {
    pub status: TslStatus,
    pub time: FdbTime,
    /// Log length, aligned by FDB_WRITE_GRAN.
    pub log_len: u32,
    pub addr: TslNodeAddr,
}

/// TSL node addresses.
#[derive(Debug, Clone, Default)]
pub struct TslNodeAddr {
    pub index: u32,
    pub log: u32,
}

impl Default for TslNode {
    fn default() -> Self {
        TslNode {
            status: TslStatus::Unused,
            time: 0,
            log_len: 0,
            addr: TslNodeAddr::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Sector info
// ---------------------------------------------------------------------------

/// KVDB sector information, matching C `struct kvdb_sec_info`.
#[derive(Debug, Clone, Copy)]
pub struct KvdbSecInfo {
    pub check_ok: bool,
    pub store: SectorStoreStatus,
    pub dirty: SectorDirtyStatus,
    pub addr: u32,
    pub magic: u32,
    /// Combined next sector number; 0xFFFFFFFF means not combined.
    pub combined: u32,
    pub remain: usize,
    pub empty_kv: u32,
}

impl Default for KvdbSecInfo {
    fn default() -> Self {
        KvdbSecInfo {
            check_ok: false,
            store: SectorStoreStatus::Unused,
            dirty: SectorDirtyStatus::Unused,
            addr: 0,
            magic: 0,
            combined: u32::MAX,
            remain: 0,
            empty_kv: 0,
        }
    }
}

/// TSDB sector information, matching C `struct tsdb_sec_info`.
#[derive(Debug, Clone)]
pub struct TsdbSecInfo {
    pub check_ok: bool,
    pub status: SectorStoreStatus,
    pub addr: u32,
    pub magic: u32,
    pub start_time: FdbTime,
    pub end_time: FdbTime,
    pub end_idx: u32,
    pub end_info_stat: [TslStatus; 2],
    pub remain: usize,
    pub empty_idx: u32,
    pub empty_data: u32,
}

impl Default for TsdbSecInfo {
    fn default() -> Self {
        TsdbSecInfo {
            check_ok: false,
            status: SectorStoreStatus::Unused,
            addr: 0,
            magic: 0,
            start_time: 0,
            end_time: 0,
            end_idx: u32::MAX,
            end_info_stat: [TslStatus::Unused; 2],
            remain: 0,
            empty_idx: 0,
            empty_data: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// KV cache node
// ---------------------------------------------------------------------------

/// KV cache node for performance optimization.
#[derive(Debug, Clone, Copy, Default)]
pub struct KvCacheNode {
    /// KV name's CRC32 low 16-bit value.
    pub name_crc: u16,
    /// KV node access active degree.
    pub active: u16,
    /// KV node address.
    pub addr: u32,
}

// ---------------------------------------------------------------------------
// Flash database base structure
// ---------------------------------------------------------------------------

/// Base database structure, matching C `struct fdb_db`.
pub struct FlashDb {
    pub name: String,
    pub db_type: FdbDbType,
    pub storage_dir: Option<String>,
    pub sec_size: u32,
    pub max_size: u32,
    pub oldest_addr: u32,
    pub init_ok: bool,
    pub file_mode: bool,
    pub not_formatable: bool,
    pub cur_file_sec: [u32; FDB_FILE_CACHE_TABLE_SIZE],
    pub cur_sec: u32,
    pub user_data: Option<Box<dyn std::any::Any>>,
}

impl Default for FlashDb {
    fn default() -> Self {
        FlashDb {
            name: String::new(),
            db_type: FdbDbType::Kv,
            storage_dir: None,
            sec_size: 0,
            max_size: 0,
            oldest_addr: 0,
            init_ok: false,
            file_mode: false,
            not_formatable: false,
            cur_file_sec: [FDB_FAILED_ADDR; FDB_FILE_CACHE_TABLE_SIZE],
            cur_sec: 0,
            user_data: None,
        }
    }
}

// ---------------------------------------------------------------------------
// KVDB structure
// ---------------------------------------------------------------------------

/// Key-value database, matching C `struct fdb_kvdb`.
pub struct KvDb<F: FlashBackend> {
    pub parent: FlashDb,
    pub default_kvs: DefaultKv,
    pub gc_request: bool,
    pub in_recovery_check: bool,
    pub cur_kv: KvNode,
    pub cur_sector: KvdbSecInfo,
    pub last_is_complete_del: bool,
    pub kv_cache_table: [KvCacheNode; FDB_KV_CACHE_TABLE_SIZE],
    pub sector_cache_table: [KvdbSecInfo; FDB_SECTOR_CACHE_TABLE_SIZE],
    pub flash: F,
}

// ---------------------------------------------------------------------------
// TSDB structure
// ---------------------------------------------------------------------------

/// Time-series database, matching C `struct fdb_tsdb`.
pub struct TsDb<F: FlashBackend> {
    pub parent: FlashDb,
    pub cur_sec: TsdbSecInfo,
    pub last_time: FdbTime,
    pub get_time: Option<FdbGetTime>,
    pub max_len: usize,
    pub rollover: bool,
    pub flash: F,
}

// ---------------------------------------------------------------------------
// Flash backend trait
// ---------------------------------------------------------------------------

/// Trait for flash storage backends.
///
/// Replaces C's direct flash read/write/erase calls.
/// Implementations: `MemFlash` (in-memory for testing), `PosixFlash` (file-based).
pub trait FlashBackend {
    /// Read `size` bytes starting at `addr`.
    fn read(&self, addr: u32, buf: &mut [u8]) -> Result<(), FdbErr>;
    /// Erase the sector containing `addr` for `size` bytes.
    fn erase(&mut self, addr: u32, size: usize) -> Result<(), FdbErr>;
    /// Write `data` starting at `addr`.
    fn write(&mut self, addr: u32, data: &[u8]) -> Result<(), FdbErr>;
    /// Get the sector size for this backend.
    fn sector_size(&self) -> u32;
}

// ---------------------------------------------------------------------------
// Low-level constants and macros (from fdb_low_lvl.h)
// ---------------------------------------------------------------------------

/// Byte value representing erased flash.
pub const FDB_BYTE_ERASED: u8 = 0xFF;

/// Byte value representing written flash.
pub const FDB_BYTE_WRITTEN: u8 = 0x00;

/// 32-bit value representing unused/erased data.
pub const FDB_DATA_UNUSED: u32 = 0xFFFFFFFF;

/// Invalid/failed address sentinel.
pub const FDB_FAILED_ADDR: u32 = 0xFFFFFFFF;

/// Align `size` up to the next multiple of `align`.
///
/// Mirrors C `FDB_ALIGN(size, align)`.
#[inline]
pub const fn fdb_align(size: u32, align: u32) -> u32 {
    ((size + align - 1) / align) * align
}

/// Align `size` down to the previous multiple of `align`.
///
/// Mirrors C `FDB_ALIGN_DOWN(size, align)`.
#[inline]
pub const fn fdb_align_down(size: u32, align: u32) -> u32 {
    (size / align) * align
}

/// Align by write granularity (round up).
///
/// Mirrors C `FDB_WG_ALIGN(size)`.
#[inline]
pub const fn fdb_wg_align(size: u32) -> u32 {
    fdb_align(size, (FDB_WRITE_GRAN as u32 + 7) / 8)
}

/// Align down by write granularity.
///
/// Mirrors C `FDB_WG_ALIGN_DOWN(size)`.
#[inline]
pub const fn fdb_wg_align_down(size: u32) -> u32 {
    fdb_align_down(size, (FDB_WRITE_GRAN as u32 + 7) / 8)
}

/// Compute the status table size in bytes for a given number of statuses.
///
/// Mirrors C `FDB_STATUS_TABLE_SIZE(status_number)`.
#[inline]
pub const fn fdb_status_table_size(status_number: usize) -> usize {
    if FDB_WRITE_GRAN == 1 {
        (status_number * FDB_WRITE_GRAN + 7) / 8
    } else {
        ((status_number - 1) * FDB_WRITE_GRAN + 7) / 8
    }
}

/// Store status table size in bytes.
pub const FDB_STORE_STATUS_TABLE_SIZE: usize = fdb_status_table_size(FDB_SECTOR_STORE_STATUS_NUM);

/// Dirty status table size in bytes.
pub const FDB_DIRTY_STATUS_TABLE_SIZE: usize = fdb_status_table_size(FDB_SECTOR_DIRTY_STATUS_NUM);

// ---------------------------------------------------------------------------
// On-flash layout constants for FDB_WRITE_GRAN=1
// ---------------------------------------------------------------------------
// Sector header #[repr(C)] layout:
//   offset 0:  status_table.store [FDB_STORE_STATUS_TABLE_SIZE=1 byte]
//   offset 1:  status_table.dirty  [FDB_DIRTY_STATUS_TABLE_SIZE=1 byte]
//   offset 2:  padding [2 bytes for u32 alignment]
//   offset 4:  magic     [u32]
//   offset 8:  combined  [u32]
//   offset 12: reserved  [u32]
//   Total: 16 bytes, WG_ALIGN(16)=16

/// Sector header on-flash offsets (matching C offsetof).
pub const SECTOR_STORE_OFFSET: usize = 0;
pub const SECTOR_DIRTY_OFFSET: usize = FDB_STORE_STATUS_TABLE_SIZE; // 1
pub const SECTOR_MAGIC_OFFSET: usize =
    SECTOR_STORE_OFFSET + FDB_STORE_STATUS_TABLE_SIZE + FDB_DIRTY_STATUS_TABLE_SIZE + 2; // padding to u32 = 4
pub const SECTOR_COMBINED_OFFSET: usize = SECTOR_MAGIC_OFFSET + 4; // 8
pub const SECTOR_RESERVED_OFFSET: usize = SECTOR_COMBINED_OFFSET + 4; // 12
/// Sector header data size = FDB_WG_ALIGN(sizeof(struct sector_hdr_data)) = 16 for GRAN=1.
pub const SECTOR_HDR_DATA_SIZE: u32 = 16;

// KV header #[repr(C)] layout:
//   offset 0:  status_table [KV_STATUS_TABLE_SIZE=1 byte]
//   offset 1:  padding [3 bytes for u32 alignment]
//   offset 4:  magic     [u32]
//   offset 8:  len       [u32]
//   offset 12: crc32     [u32]
//   offset 16: name_len  [u8]
//   offset 17: padding [3 bytes for u32 alignment]
//   offset 20: value_len [u32]
//   Total: 24 bytes, WG_ALIGN(24)=24

/// KV header on-flash offsets (matching C offsetof).
pub const KV_STATUS_TABLE_SIZE: usize = fdb_status_table_size(FDB_KV_STATUS_NUM); // 1
pub const KV_MAGIC_OFFSET: usize = 4;
pub const KV_LEN_OFFSET: usize = 8;
pub const KV_CRC_OFFSET: usize = 12;
pub const KV_NAME_LEN_OFFSET: usize = 16;
pub const KV_VALUE_LEN_OFFSET: usize = 20;
/// KV header data size = FDB_WG_ALIGN(sizeof(struct kv_hdr_data)) = 24 for GRAN=1.
pub const KV_HDR_DATA_SIZE: u32 = 24;

/// TSL header on-flash offsets.
pub const TSL_HDR_STATUS_SZ: usize = fdb_status_table_size(FDB_TSL_STATUS_NUM);
pub const TSL_HDR_SZ: u32 = fdb_wg_align((TSL_HDR_STATUS_SZ + 4 + 4 + 4) as u32);

// ---------------------------------------------------------------------------
// TSDB-specific on-flash layout constants (from fdb_tsdb.c)
// ---------------------------------------------------------------------------
// TSDB sector header #[repr(C)] layout (GRAN=1, no 64-bit timestamp, no fixed blob):
//   offset 0:  status[1]           (FDB_STORE_STATUS_TABLE_SIZE=1)
//   offset 1:  magic[4]            (TSL_UINT32_ALIGN_SIZE=4, uint8_t[])
//   offset 5:  start_time[4]       (TSL_TIME_ALIGN_SIZE=4, uint8_t[])
//   offset 9:  end_info[0].time[4]
//   offset 13: end_info[0].index[4]
//   offset 17: end_info[0].status[1]
//   offset 18: end_info[1].time[4]
//   offset 22: end_info[1].index[4]
//   offset 26: end_info[1].status[1]
//   offset 27: (1 byte padding for u32 alignment)
//   offset 28: reserved [u32]
//   Total: 32 bytes, WG_ALIGN(32)=32

/// TSDB sector magic word: 'T','S','L','0' = 0x304C5354.
pub const TSDB_SECTOR_MAGIC_WORD: u32 = 0x304C5354;
/// TSDB sector header on-flash offsets (matching C offsetof).
pub const TSDB_SECTOR_MAGIC_OFFSET: usize = 1;
pub const TSDB_SECTOR_START_TIME_OFFSET: usize = 5;
pub const TSDB_SECTOR_END0_TIME_OFFSET: usize = 9;
pub const TSDB_SECTOR_END0_IDX_OFFSET: usize = 13;
pub const TSDB_SECTOR_END0_STATUS_OFFSET: usize = 17;
pub const TSDB_SECTOR_END1_TIME_OFFSET: usize = 18;
pub const TSDB_SECTOR_END1_IDX_OFFSET: usize = 22;
pub const TSDB_SECTOR_END1_STATUS_OFFSET: usize = 26;
/// TSDB sector header data size = FDB_WG_ALIGN(sizeof(struct sector_hdr_data)) = 32 for GRAN=1.
pub const TSDB_SECTOR_HDR_DATA_SIZE: u32 = 32;
/// TSDB sector header buffer size for writes.
pub const TSDB_SEC_HDR_BUF_SZ: usize = TSDB_SECTOR_HDR_DATA_SIZE as usize;

// TSL log index #[repr(C)] layout (GRAN=1, no fixed blob, no 64-bit timestamp):
//   offset 0:  status_table[1]  (TSL_STATUS_TABLE_SIZE=1)
//   offset 1:  (3 bytes padding for i32 alignment)
//   offset 4:  time [i32]
//   offset 8:  log_len [u32]
//   offset 12: log_addr [u32]
//   Total: 16 bytes, WG_ALIGN(16)=16

/// TSL log index data size = FDB_WG_ALIGN(sizeof(struct log_idx_data)) = 16 for GRAN=1.
pub const LOG_IDX_DATA_SIZE: u32 = 16;
/// TSL log index: offset of time field.
pub const LOG_IDX_TS_OFFSET: usize = 4;
/// TSL status table size in bytes.
pub const TSL_STATUS_TABLE_SIZE: usize = fdb_status_table_size(FDB_TSL_STATUS_NUM);

// ---------------------------------------------------------------------------
// KVDB-specific constants from fdb_kvdb.c:34-68
// ---------------------------------------------------------------------------

/// Sector magic word: 'F','D','B','1' = 0x30424446.
pub const SECTOR_MAGIC_WORD: u32 = 0x30424446;
/// KV magic word: 'K','V','0','0' = 0x3030564B.
pub const KV_MAGIC_WORD: u32 = 0x3030564B;
/// Sector not combined sentinel.
pub const SECTOR_NOT_COMBINED: u32 = 0xFFFFFFFF;
/// Sector combined sentinel.
pub const SECTOR_COMBINED: u32 = 0x00000000;
/// GC minimum number of empty sectors.
pub const FDB_GC_EMPTY_SEC_THRESHOLD: usize = 1;
/// Sector remain threshold before full status.
pub const FDB_SEC_REMAIN_THRESHOLD: usize =
    KV_HDR_DATA_SIZE as usize + crate::config::FDB_KV_NAME_MAX;
/// String KV value max buffer size.
pub const FDB_STR_KV_VALUE_MAX_SIZE: usize = 128;
/// Version number KV name.
pub const VER_NUM_KV_NAME: &str = "__ver_num__";

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FDB_SW_VERSION, FDB_SW_VERSION_NUM};

    #[test]
    fn fdb_err_no_err_is_default_like() {
        assert_eq!(FdbErr::default(), FdbErr::NoErr);
    }

    #[test]
    fn fdb_err_display_all_variants() {
        assert_eq!(FdbErr::NoErr.to_string(), "NoErr");
        assert_eq!(FdbErr::EraseErr.to_string(), "EraseErr");
        assert_eq!(FdbErr::InitFailed.to_string(), "InitFailed");
    }

    #[test]
    fn kv_status_variants() {
        assert_eq!(FDB_KV_STATUS_NUM, 6);
        let s = KvStatus::default();
        assert_eq!(s, KvStatus::Unused);
    }

    #[test]
    fn tsl_status_variants() {
        assert_eq!(FDB_TSL_STATUS_NUM, 6);
        let s = TslStatus::default();
        assert_eq!(s, TslStatus::Unused);
    }

    #[test]
    fn sector_store_status_variants() {
        assert_eq!(FDB_SECTOR_STORE_STATUS_NUM, 4);
    }

    #[test]
    fn sector_dirty_status_variants() {
        assert_eq!(FDB_SECTOR_DIRTY_STATUS_NUM, 4);
    }

    #[test]
    fn failed_addr_constant() {
        assert_eq!(FDB_FAILED_ADDR, 0xFFFFFFFF);
    }

    #[test]
    fn data_unused_constant() {
        assert_eq!(FDB_DATA_UNUSED, 0xFFFFFFFF);
    }

    #[test]
    fn version_constants() {
        assert_eq!(FDB_SW_VERSION, "2.2.99");
        assert_eq!(FDB_SW_VERSION_NUM, 0x20299);
    }

    #[test]
    fn blob_default() {
        let b = Blob::make(&[]);
        assert!(b.buf.is_empty());
        assert_eq!(b.saved.meta_addr, 0);
        assert_eq!(b.saved.addr, 0);
        assert_eq!(b.saved.len, 0);
    }

    #[test]
    fn blob_make_creates_blob() {
        let b = Blob::make(&[1, 2, 3]);
        assert_eq!(b.buf, vec![1, 2, 3]);
    }

    #[test]
    fn kv_node_default() {
        let kv = KvNode::default();
        assert_eq!(kv.status, KvStatus::Unused);
        assert!(!kv.crc_is_ok);
        assert_eq!(kv.name, "");
    }

    #[test]
    fn kv_db_default() {
        let db = FlashDb::default();
        assert!(!db.init_ok);
        assert!(!db.file_mode);
        assert_eq!(db.sec_size, 0);
    }

    #[test]
    fn tsl_node_default() {
        let tsl = TslNode::default();
        assert_eq!(tsl.status, TslStatus::Unused);
        assert_eq!(tsl.time, 0);
    }

    #[test]
    fn flash_db_default() {
        let db = FlashDb::default();
        assert!(!db.init_ok);
    }

    #[test]
    fn kv_iterator_default() {
        let itr = KvIterator::default();
        assert_eq!(itr.iterated_cnt, 0);
    }

    #[test]
    fn kvdb_sec_info_default() {
        let s = KvdbSecInfo::default();
        assert!(!s.check_ok);
        assert_eq!(s.combined, u32::MAX);
    }

    #[test]
    fn tsdb_sec_info_default() {
        let s = TsdbSecInfo::default();
        assert!(!s.check_ok);
        assert_eq!(s.end_idx, u32::MAX);
    }
}
