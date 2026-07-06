//! KVDB (Key-Value Database) feature implementation.
//!
//! 1:1 behavioral translation of `fdb_kvdb.c`.

use crate::config::{
    FDB_KV_CACHE_TABLE_SIZE, FDB_KV_NAME_MAX, FDB_KV_USING_CACHE, FDB_SECTOR_CACHE_TABLE_SIZE,
    FDB_WRITE_GRAN,
};
use crate::lowlevel;
use crate::types::{
    fdb_align_down, fdb_wg_align, Blob, BlobSaved, DefaultKv, FdbDbType, FdbErr, FlashBackend,
    FlashDb, KvCacheNode, KvControlArg, KvDb, KvIterator, KvNode, KvNodeAddr, KvStatus,
    KvdbSecInfo, SectorDirtyStatus, SectorStoreStatus, FDB_DATA_UNUSED, FDB_FAILED_ADDR,
    FDB_GC_EMPTY_SEC_THRESHOLD, FDB_KV_STATUS_NUM, FDB_SECTOR_DIRTY_STATUS_NUM,
    FDB_SECTOR_STORE_STATUS_NUM, FDB_SEC_REMAIN_THRESHOLD, FDB_STR_KV_VALUE_MAX_SIZE,
    KV_HDR_DATA_SIZE, KV_MAGIC_OFFSET, KV_MAGIC_WORD, SECTOR_COMBINED, SECTOR_DIRTY_OFFSET,
    SECTOR_HDR_DATA_SIZE, SECTOR_MAGIC_OFFSET, SECTOR_MAGIC_WORD, SECTOR_NOT_COMBINED,
    SECTOR_STORE_OFFSET,
};
use crate::utils::{
    calc_crc32, continue_ff_addr, flash_write_align, get_status, read_status, set_status,
    write_status,
};

// ---------------------------------------------------------------------------
// On-flash layout helpers
// ---------------------------------------------------------------------------

fn read_u32_le(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn write_u32_le(buf: &mut [u8], off: usize, val: u32) {
    let bytes = val.to_le_bytes();
    buf[off] = bytes[0];
    buf[off + 1] = bytes[1];
    buf[off + 2] = bytes[2];
    buf[off + 3] = bytes[3];
}

fn write_u8(buf: &mut [u8], off: usize, val: u8) {
    buf[off] = val;
}

const SEC_HDR_BUF_SZ: usize = SECTOR_HDR_DATA_SIZE as usize;
const KV_HDR_BUF_SZ: usize = KV_HDR_DATA_SIZE as usize;

const KV_NAME_LEN_OFF: usize = 16;
const KV_VALUE_LEN_OFF: usize = 20;
const KV_CRC_OFF: usize = 12;

// ---------------------------------------------------------------------------
// Cache helpers — fdb_kvdb.c:150-275
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn update_sector_cache(&mut self, sector: &KvdbSecInfo) {
        if !FDB_KV_USING_CACHE {
            return;
        }
        let mut empty_index = FDB_SECTOR_CACHE_TABLE_SIZE;
        for i in 0..FDB_SECTOR_CACHE_TABLE_SIZE {
            if self.sector_cache_table[i].addr == sector.addr {
                if sector.check_ok {
                    self.sector_cache_table[i] = *sector;
                } else {
                    self.sector_cache_table[i].addr = FDB_DATA_UNUSED;
                }
                return;
            } else if self.sector_cache_table[i].addr == FDB_DATA_UNUSED
                && empty_index == FDB_SECTOR_CACHE_TABLE_SIZE
            {
                empty_index = i;
            }
        }
        if sector.check_ok && empty_index < FDB_SECTOR_CACHE_TABLE_SIZE {
            self.sector_cache_table[empty_index] = *sector;
        }
    }

    fn get_sector_cache_idx(&self, sec_addr: u32) -> Option<usize> {
        if !FDB_KV_USING_CACHE {
            return None;
        }
        for i in 0..FDB_SECTOR_CACHE_TABLE_SIZE {
            if self.sector_cache_table[i].addr == sec_addr {
                return Some(i);
            }
        }
        None
    }

    fn update_sector_empty_addr_cache(&mut self, sec_addr: u32, empty_addr: u32) {
        if !FDB_KV_USING_CACHE {
            return;
        }
        if let Some(idx) = self.get_sector_cache_idx(sec_addr) {
            self.sector_cache_table[idx].empty_kv = empty_addr;
            self.sector_cache_table[idx].remain = self.parent.sec_size as usize
                - (empty_addr - self.sector_cache_table[idx].addr) as usize;
        }
    }

    fn update_sector_status_store_cache(&mut self, sec_addr: u32, store: SectorStoreStatus) {
        if !FDB_KV_USING_CACHE {
            return;
        }
        if let Some(idx) = self.get_sector_cache_idx(sec_addr) {
            self.sector_cache_table[idx].store = store;
        }
    }

    fn update_kv_cache(&mut self, name: &str, _name_len: usize, addr: u32) {
        if !FDB_KV_USING_CACHE {
            return;
        }
        let name_crc = (calc_crc32(0, name.as_bytes()) >> 16) as u16;
        let mut empty_index = FDB_KV_CACHE_TABLE_SIZE;
        let mut min_activity_index = FDB_KV_CACHE_TABLE_SIZE;
        let mut min_activity: u16 = 0xFFFF;

        for i in 0..FDB_KV_CACHE_TABLE_SIZE {
            if addr != FDB_DATA_UNUSED {
                if self.kv_cache_table[i].name_crc == name_crc {
                    self.kv_cache_table[i].addr = addr;
                    return;
                } else if self.kv_cache_table[i].addr == FDB_DATA_UNUSED
                    && empty_index == FDB_KV_CACHE_TABLE_SIZE
                {
                    empty_index = i;
                } else if self.kv_cache_table[i].addr != FDB_DATA_UNUSED {
                    if self.kv_cache_table[i].active > 0 {
                        self.kv_cache_table[i].active -= 1;
                    }
                    if self.kv_cache_table[i].active < min_activity {
                        min_activity_index = i;
                        min_activity = self.kv_cache_table[i].active;
                    }
                }
            } else if self.kv_cache_table[i].name_crc == name_crc {
                self.kv_cache_table[i].addr = FDB_DATA_UNUSED;
                self.kv_cache_table[i].active = 0;
                return;
            }
        }
        if empty_index < FDB_KV_CACHE_TABLE_SIZE {
            self.kv_cache_table[empty_index].addr = addr;
            self.kv_cache_table[empty_index].name_crc = name_crc;
            self.kv_cache_table[empty_index].active = FDB_KV_CACHE_TABLE_SIZE as u16;
        } else if min_activity_index < FDB_KV_CACHE_TABLE_SIZE {
            self.kv_cache_table[min_activity_index].addr = addr;
            self.kv_cache_table[min_activity_index].name_crc = name_crc;
            self.kv_cache_table[min_activity_index].active = FDB_KV_CACHE_TABLE_SIZE as u16;
        }
    }

    fn get_kv_from_cache(&mut self, name: &str, name_len: usize) -> Option<u32> {
        if !FDB_KV_USING_CACHE {
            return None;
        }
        let name_crc = (calc_crc32(0, name.as_bytes()) >> 16) as u16;
        for i in 0..FDB_KV_CACHE_TABLE_SIZE {
            if self.kv_cache_table[i].addr != FDB_DATA_UNUSED
                && self.kv_cache_table[i].name_crc == name_crc
            {
                let kv_addr = self.kv_cache_table[i].addr;
                let mut saved_name = vec![0u8; FDB_KV_NAME_MAX];
                if self
                    .flash
                    .read(kv_addr + KV_HDR_DATA_SIZE, &mut saved_name)
                    .is_ok()
                {
                    if name.as_bytes().get(..name_len) == saved_name.get(..name_len) {
                        if self.kv_cache_table[i].active
                            >= (0xFFFF - FDB_KV_CACHE_TABLE_SIZE as u16)
                        {
                            self.kv_cache_table[i].active = 0xFFFF;
                        } else {
                            self.kv_cache_table[i].active += FDB_KV_CACHE_TABLE_SIZE as u16;
                        }
                        return Some(kv_addr);
                    }
                }
            }
        }
        None
    }

    fn invalidate_kv_cache(&mut self, name: &str, name_len: usize) {
        self.update_kv_cache(name, name_len, FDB_DATA_UNUSED);
    }

    fn sector_num(&self) -> u32 {
        self.parent.max_size / self.parent.sec_size
    }
}

// ---------------------------------------------------------------------------
// find_next_kv_addr — fdb_kvdb.c:280-310
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn find_next_kv_addr(&self, start: u32, end: u32) -> u32 {
        let mut start = start;
        let start_bak = start;
        let buf_sz: u32 = 32;

        if FDB_KV_USING_CACHE {
            if let Some(idx) =
                self.get_sector_cache_idx(fdb_align_down(start, self.parent.sec_size))
            {
                if start == self.sector_cache_table[idx].empty_kv {
                    return FDB_FAILED_ADDR;
                }
            }
        }

        while start < end && start + buf_sz < end {
            let mut buf = [0u8; 32];
            if self.flash.read(start, &mut buf).is_err() {
                return FDB_FAILED_ADDR;
            }
            for i in 0..(buf_sz as usize - 4) {
                let addr = start + i as u32;
                if addr >= end {
                    break;
                }
                let magic = u32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]);
                if magic == KV_MAGIC_WORD && addr >= start_bak + KV_MAGIC_OFFSET as u32 {
                    let kv_addr = addr - KV_MAGIC_OFFSET as u32;
                    if kv_addr >= start_bak {
                        return kv_addr;
                    }
                }
            }
            start += buf_sz - 4;
        }

        FDB_FAILED_ADDR
    }
}

// ---------------------------------------------------------------------------
// get_next_kv_addr — fdb_kvdb.c:312-346
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn get_next_kv_addr(&self, sector: &KvdbSecInfo, pre_kv: &KvNode) -> u32 {
        if sector.store == SectorStoreStatus::Empty {
            return FDB_FAILED_ADDR;
        }

        let addr = if pre_kv.addr.start == FDB_FAILED_ADDR {
            sector.addr + SECTOR_HDR_DATA_SIZE
        } else if pre_kv.addr.start <= sector.addr + self.parent.sec_size {
            if pre_kv.crc_is_ok {
                pre_kv.addr.start + pre_kv.len
            } else {
                pre_kv.addr.start + fdb_wg_align(1)
            }
        } else {
            return FDB_FAILED_ADDR;
        };

        let search_end = sector.addr + self.parent.sec_size - SECTOR_HDR_DATA_SIZE;
        if search_end <= addr {
            return FDB_FAILED_ADDR;
        }

        let found = self.find_next_kv_addr(addr, search_end);
        if found == FDB_FAILED_ADDR || found > sector.addr + self.parent.sec_size || pre_kv.len == 0
        {
            return FDB_FAILED_ADDR;
        }
        found
    }
}

// ---------------------------------------------------------------------------
// read_kv — fdb_kvdb.c:348-414
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn read_kv(&mut self, kv: &mut KvNode) -> Result<(), FdbErr> {
        let mut hdr_buf = vec![0xFFu8; KV_HDR_BUF_SZ];
        if self.flash.read(kv.addr.start, &mut hdr_buf).is_err() {
            kv.crc_is_ok = false;
            return Err(FdbErr::ReadErr);
        }

        let status_byte = hdr_buf[0];
        let status_idx = get_status(&[status_byte], FDB_KV_STATUS_NUM as usize);
        kv.status = match status_idx {
            0 => KvStatus::Unused,
            1 => KvStatus::PreWrite,
            2 => KvStatus::Write,
            3 => KvStatus::PreDelete,
            4 => KvStatus::Deleted,
            _ => KvStatus::ErrHdr,
        };

        kv.len = read_u32_le(&hdr_buf, KV_MAGIC_OFFSET as usize + 4);

        if kv.len == u32::MAX || kv.len > self.parent.max_size || kv.len < KV_HDR_DATA_SIZE {
            kv.len = KV_HDR_DATA_SIZE;
            if kv.status != KvStatus::ErrHdr {
                kv.status = KvStatus::ErrHdr;
                eprintln!(
                    "Error: The KV @0x{:08X} length has an error.",
                    kv.addr.start
                );
                let mut st = [0xFFu8; 1];
                set_status(&mut st, FDB_KV_STATUS_NUM as usize, 5);
                let _ = self.flash.write(kv.addr.start, &st[..1]);
            }
            kv.crc_is_ok = false;
            return Err(FdbErr::ReadErr);
        }

        let name_len_raw = hdr_buf[KV_NAME_LEN_OFF];
        let value_len_raw = read_u32_le(&hdr_buf, KV_VALUE_LEN_OFF);

        let crc_data_len = kv.len - KV_HDR_DATA_SIZE;
        let mut calc_crc: u32 = 0;
        calc_crc = calc_crc32(calc_crc, &hdr_buf[KV_NAME_LEN_OFF..KV_NAME_LEN_OFF + 4]);
        calc_crc = calc_crc32(calc_crc, &hdr_buf[KV_VALUE_LEN_OFF..KV_VALUE_LEN_OFF + 4]);

        let mut len: usize = 0;
        while len < crc_data_len as usize {
            let size = if len + 32 < crc_data_len as usize {
                32
            } else {
                crc_data_len as usize - len
            };
            let read_size = fdb_wg_align(size as u32) as usize;
            let mut read_buf = vec![0u8; read_size];
            if self
                .flash
                .read(kv.addr.start + KV_HDR_DATA_SIZE + len as u32, &mut read_buf)
                .is_err()
            {
                kv.crc_is_ok = false;
                return Err(FdbErr::ReadErr);
            }
            calc_crc = calc_crc32(calc_crc, &read_buf[..size]);
            len += size;
        }

        let hdr_crc = read_u32_le(&hdr_buf, KV_CRC_OFF);

        if calc_crc != hdr_crc {
            let name_len = name_len_raw as usize;
            let name_len = if name_len > FDB_KV_NAME_MAX {
                FDB_KV_NAME_MAX
            } else {
                name_len
            };
            kv.crc_is_ok = false;
            let mut name_buf = vec![0u8; fdb_wg_align(name_len as u32) as usize];
            let _ = self
                .flash
                .read(kv.addr.start + KV_HDR_DATA_SIZE, &mut name_buf);
            let name_str = String::from_utf8_lossy(&name_buf[..name_len]).to_string();
            eprintln!(
                "Error: Read the KV ({}@0x{:08X}) CRC32 check failed!",
                name_str, kv.addr.start
            );
            kv.name = name_str;
            Err(FdbErr::ReadErr)
        } else {
            kv.crc_is_ok = true;
            let name_len = name_len_raw as usize;
            let name_len = if name_len > FDB_KV_NAME_MAX {
                FDB_KV_NAME_MAX
            } else {
                name_len
            };
            let aligned_name_len = fdb_wg_align(name_len as u32) as usize;
            let mut name_buf = vec![0u8; aligned_name_len];
            self.flash
                .read(kv.addr.start + KV_HDR_DATA_SIZE, &mut name_buf)?;
            kv.addr.value = kv.addr.start + KV_HDR_DATA_SIZE + aligned_name_len as u32;
            kv.value_len = value_len_raw;
            kv.name_len = name_len_raw;
            kv.name = String::from_utf8_lossy(&name_buf[..name_len]).to_string();
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// read_sector_info — fdb_kvdb.c:416-501
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn read_sector_info(&mut self, addr: u32, traversal: bool) -> Result<KvdbSecInfo, FdbErr> {
        assert_eq!(
            addr % self.parent.sec_size,
            0,
            "addr must be sector-aligned"
        );

        if FDB_KV_USING_CACHE {
            if let Some(idx) = self.get_sector_cache_idx(addr) {
                let cached = self.sector_cache_table[idx];
                if !traversal || (traversal && cached.empty_kv != FDB_FAILED_ADDR) {
                    return Ok(cached);
                }
            }
        }

        let mut sec_hdr = vec![0xFFu8; SEC_HDR_BUF_SZ];
        let _ = self.flash.read(addr, &mut sec_hdr);

        let magic = read_u32_le(&sec_hdr, SECTOR_MAGIC_OFFSET as usize);
        let combined_val = read_u32_le(&sec_hdr, SECTOR_MAGIC_OFFSET as usize + 4);

        let mut sector = KvdbSecInfo {
            store: SectorStoreStatus::Unused,
            dirty: SectorDirtyStatus::Unused,
            addr,
            magic,
            check_ok: false,
            combined: SECTOR_NOT_COMBINED,
            remain: 0,
            empty_kv: 0,
        };

        if magic != SECTOR_MAGIC_WORD
            || (combined_val != SECTOR_NOT_COMBINED && combined_val != SECTOR_COMBINED)
        {
            if FDB_KV_USING_CACHE && !traversal {
                if self.get_sector_cache_idx(sector.addr).is_none() {
                    sector.empty_kv = FDB_FAILED_ADDR;
                    sector.remain = 0;
                    self.update_sector_cache(&sector);
                }
            }
            return Ok(sector);
        }

        sector.check_ok = true;
        sector.combined = combined_val;

        let store_status = read_status(
            &self.flash,
            addr + SECTOR_STORE_OFFSET as u32,
            FDB_SECTOR_STORE_STATUS_NUM as usize,
        );
        sector.store = match store_status {
            1 => SectorStoreStatus::Empty,
            2 => SectorStoreStatus::Using,
            3 => SectorStoreStatus::Full,
            _ => SectorStoreStatus::Unused,
        };

        let dirty_status = read_status(
            &self.flash,
            addr + SECTOR_DIRTY_OFFSET as u32,
            FDB_SECTOR_DIRTY_STATUS_NUM as usize,
        );
        sector.dirty = match dirty_status {
            1 => SectorDirtyStatus::False,
            2 => SectorDirtyStatus::True,
            3 => SectorDirtyStatus::GC,
            _ => SectorDirtyStatus::Unused,
        };

        if traversal {
            sector.remain = 0;
            sector.empty_kv = sector.addr + SECTOR_HDR_DATA_SIZE;

            if sector.store == SectorStoreStatus::Empty {
                sector.remain = self.parent.sec_size as usize - SECTOR_HDR_DATA_SIZE as usize;
            } else if sector.store == SectorStoreStatus::Using {
                sector.remain = self.parent.sec_size as usize - SECTOR_HDR_DATA_SIZE as usize;
                let mut kv_obj = KvNode::default();
                kv_obj.addr.start = sector.addr + SECTOR_HDR_DATA_SIZE;

                loop {
                    let _ = self.read_kv(&mut kv_obj);
                    if !kv_obj.crc_is_ok {
                        if kv_obj.status != KvStatus::PreWrite && kv_obj.status != KvStatus::ErrHdr
                        {
                            sector.remain = 0;
                            break;
                        }
                    }
                    sector.empty_kv += kv_obj.len;
                    if sector.remain >= kv_obj.len as usize {
                        sector.remain -= kv_obj.len as usize;
                    } else {
                        sector.remain = 0;
                    }
                    let next = self.get_next_kv_addr(&sector, &kv_obj);
                    if next == FDB_FAILED_ADDR {
                        break;
                    }
                    kv_obj.addr.start = next;
                }

                let ff_addr = continue_ff_addr(
                    &self.flash,
                    sector.empty_kv,
                    sector.addr + self.parent.sec_size,
                );
                if sector.empty_kv != ff_addr {
                    sector.empty_kv = ff_addr;
                    sector.remain =
                        self.parent.sec_size as usize - (ff_addr - sector.addr) as usize;
                }
            }

            if FDB_KV_USING_CACHE {
                self.update_sector_cache(&sector);
            }
        } else if FDB_KV_USING_CACHE && self.get_sector_cache_idx(sector.addr).is_none() {
            sector.empty_kv = FDB_FAILED_ADDR;
            sector.remain = 0;
            self.update_sector_cache(&sector);
        }

        Ok(sector)
    }
}

// ---------------------------------------------------------------------------
// get_next_sector_addr — fdb_kvdb.c:504-526
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn get_next_sector_addr(&self, pre_sec: &KvdbSecInfo, traversed_len: u32) -> u32 {
        let cur_block_size = if pre_sec.combined == SECTOR_NOT_COMBINED {
            self.parent.sec_size
        } else {
            pre_sec.combined * self.parent.sec_size
        };

        if traversed_len + cur_block_size <= self.parent.max_size {
            if pre_sec.addr + cur_block_size < self.parent.max_size {
                pre_sec.addr + cur_block_size
            } else {
                0
            }
        } else {
            FDB_FAILED_ADDR
        }
    }
}

// ---------------------------------------------------------------------------
// find_kv — fdb_kvdb.c:559-607 (inline iteration, no closures)
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn find_kv_no_cache(&mut self, key: &str) -> Option<KvNode> {
        let key_bytes = key.as_bytes();
        let key_len = key_bytes.len();
        let mut sector = KvdbSecInfo::default();
        let mut sec_addr = self.parent.oldest_addr;
        let mut traversed_len: u32 = 0;

        loop {
            traversed_len += self.parent.sec_size;
            if let Ok(sec) = self.read_sector_info(sec_addr, false) {
                sector = sec;
                if sector.store == SectorStoreStatus::Using
                    || sector.store == SectorStoreStatus::Full
                {
                    let mut kv = KvNode::default();
                    kv.addr.start = sector.addr + SECTOR_HDR_DATA_SIZE;
                    loop {
                        let _ = self.read_kv(&mut kv);
                        if kv.crc_is_ok
                            && kv.status == KvStatus::Write
                            && kv.name_len as usize == key_len
                            && kv.name.as_bytes() == key_bytes
                        {
                            return Some(kv);
                        }
                        let next = self.get_next_kv_addr(&sector, &kv);
                        if next == FDB_FAILED_ADDR {
                            break;
                        }
                        kv.addr.start = next;
                    }
                }
            }
            sec_addr = self.get_next_sector_addr(&sector, traversed_len);
            if sec_addr == FDB_FAILED_ADDR {
                break;
            }
        }
        None
    }

    fn find_kv(&mut self, key: &str) -> Option<KvNode> {
        if FDB_KV_USING_CACHE {
            if let Some(cache_addr) = self.get_kv_from_cache(key, key.len()) {
                let mut kv = KvNode::default();
                kv.addr.start = cache_addr;
                if self.read_kv(&mut kv).is_ok() {
                    return Some(kv);
                }
            }
        }
        let result = self.find_kv_no_cache(key);
        if FDB_KV_USING_CACHE {
            if let Some(ref kv) = result {
                self.update_kv_cache(key, key.len(), kv.addr.start);
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// is_str — fdb_kvdb.c:609-620
// ---------------------------------------------------------------------------

fn is_str(value: &[u8]) -> bool {
    for &ch in value {
        if (ch as u32).wrapping_sub(' ' as u32) >= 127u32 - ' ' as u32 {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// get_kv — fdb_kvdb.c:622-644
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn get_kv_inner(&mut self, key: &str, value_buf: &mut [u8]) -> (usize, usize) {
        if let Some(kv) = self.find_kv(key) {
            let value_len = kv.value_len as usize;
            let read_len = if value_buf.len() < value_len {
                value_buf.len()
            } else {
                value_len
            };
            if read_len > 0 {
                let _ = self.flash.read(kv.addr.value, &mut value_buf[..read_len]);
            }
            (read_len, value_len)
        } else {
            (0, 0)
        }
    }
}

// ---------------------------------------------------------------------------
// Public get API — fdb_kvdb.c:655-753
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    pub fn kv_get_obj(&mut self, key: &str) -> Option<KvNode> {
        if !self.parent.init_ok {
            eprintln!("Error: KV ({}) isn't initialize OK.", self.parent.name);
            return None;
        }
        self.find_kv(key)
    }

    pub fn kv_get_blob(&mut self, key: &str, blob: &mut Blob) -> usize {
        if !self.parent.init_ok {
            eprintln!("Error: KV ({}) isn't initialize OK.", self.parent.name);
            return 0;
        }
        let mut buf = vec![0u8; blob.buf.len()];
        let (read_len, saved_len) = self.get_kv_inner(key, &mut buf);
        blob.saved.len = saved_len;
        if read_len > 0 {
            blob.buf[..read_len].copy_from_slice(&buf[..read_len]);
        }
        read_len
    }

    pub fn kv_get(&mut self, key: &str) -> Option<String> {
        let value_buf = vec![0u8; FDB_STR_KV_VALUE_MAX_SIZE];
        let mut blob = Blob {
            buf: value_buf,
            saved: BlobSaved::default(),
        };
        let get_size = self.kv_get_blob(key, &mut blob);
        if get_size > 0 {
            if is_str(&blob.buf[..get_size]) {
                return Some(String::from_utf8_lossy(&blob.buf[..get_size]).to_string());
            } else if blob.saved.len > FDB_STR_KV_VALUE_MAX_SIZE {
                eprintln!(
                    "Warning: The default string KV value buffer length ({}) is too less ({}).",
                    FDB_STR_KV_VALUE_MAX_SIZE, blob.saved.len
                );
            } else {
                eprintln!("Warning: The KV value isn't string. Could not be returned");
                return None;
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// write_kv_hdr — fdb_kvdb.c:755-767
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn write_kv_hdr(&mut self, addr: u32, hdr_buf: &[u8]) -> Result<(), FdbErr> {
        write_status(
            &mut self.parent,
            &mut self.flash,
            addr,
            &[0xFFu8; 1],
            FDB_KV_STATUS_NUM as usize,
            1,
            false,
        )?;
        let rest_start = KV_MAGIC_OFFSET as usize;
        let rest_end = KV_HDR_BUF_SZ;
        self.flash.write(
            addr + KV_MAGIC_OFFSET as u32,
            &hdr_buf[rest_start..rest_end],
        )?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// format_sector — fdb_kvdb.c:769-827
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn format_sector(&mut self, addr: u32, combined_value: u32) -> Result<(), FdbErr> {
        assert_eq!(
            addr % self.parent.sec_size,
            0,
            "addr must be sector-aligned"
        );
        self.flash.erase(addr, self.parent.sec_size as usize)?;

        let mut hdr_buf = vec![0xFFu8; SEC_HDR_BUF_SZ];
        set_status(&mut hdr_buf, FDB_SECTOR_STORE_STATUS_NUM as usize, 1); // Empty
        let mut dirty_byte = [0xFFu8; 1];
        set_status(&mut dirty_byte, FDB_SECTOR_DIRTY_STATUS_NUM as usize, 1); // False
        hdr_buf[1] = dirty_byte[0];
        write_u32_le(
            &mut hdr_buf,
            SECTOR_MAGIC_OFFSET as usize,
            SECTOR_MAGIC_WORD,
        );
        write_u32_le(
            &mut hdr_buf,
            SECTOR_MAGIC_OFFSET as usize + 4,
            combined_value,
        );
        write_u32_le(
            &mut hdr_buf,
            SECTOR_MAGIC_OFFSET as usize + 8,
            FDB_DATA_UNUSED,
        );
        self.flash
            .write(addr, &hdr_buf[..SECTOR_HDR_DATA_SIZE as usize])?;

        if FDB_KV_USING_CACHE {
            self.update_sector_cache(&KvdbSecInfo {
                addr,
                check_ok: false,
                empty_kv: FDB_FAILED_ADDR,
                ..KvdbSecInfo::default()
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// update_sec_status — fdb_kvdb.c:829-861
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn update_sec_status(&mut self, sector: &KvdbSecInfo, new_kv_len: u32) -> Result<bool, FdbErr> {
        let mut is_full = false;
        if sector.store == SectorStoreStatus::Empty {
            write_status(
                &mut self.parent,
                &mut self.flash,
                sector.addr,
                &[0xFFu8; 1],
                FDB_SECTOR_STORE_STATUS_NUM as usize,
                2,
                true,
            )?;
            if FDB_KV_USING_CACHE {
                self.update_sector_status_store_cache(sector.addr, SectorStoreStatus::Using);
            }
        } else if sector.store == SectorStoreStatus::Using {
            let new_kv_len_usize = new_kv_len as usize;
            if sector.remain < FDB_SEC_REMAIN_THRESHOLD
                || sector.remain - new_kv_len_usize < FDB_SEC_REMAIN_THRESHOLD
            {
                write_status(
                    &mut self.parent,
                    &mut self.flash,
                    sector.addr,
                    &[0xFFu8; 1],
                    FDB_SECTOR_STORE_STATUS_NUM as usize,
                    3,
                    true,
                )?;
                if FDB_KV_USING_CACHE {
                    self.update_sector_status_store_cache(sector.addr, SectorStoreStatus::Full);
                }
                is_full = true;
            }
        }
        Ok(is_full)
    }
}

// ---------------------------------------------------------------------------
// alloc_kv — fdb_kvdb.c:885-938 (inline iteration)
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn alloc_kv(&mut self, kv_size: usize) -> (u32, KvdbSecInfo) {
        let gc_request = self.gc_request;

        // Count empty/using sectors
        let mut empty_sector: usize = 0;
        let mut using_sector: usize = 0;
        let mut sec_addr = self.parent.oldest_addr;
        let mut sector = KvdbSecInfo::default();
        let mut traversed_len: u32 = 0;
        loop {
            traversed_len += self.parent.sec_size;
            if let Ok(sec) = self.read_sector_info(sec_addr, false) {
                sector = sec;
                if sector.check_ok {
                    match sector.store {
                        SectorStoreStatus::Empty => empty_sector += 1,
                        SectorStoreStatus::Using => using_sector += 1,
                        _ => {}
                    }
                }
            }
            let next = self.get_next_sector_addr(&sector, traversed_len);
            if next == FDB_FAILED_ADDR {
                break;
            }
            sec_addr = next;
        }

        let mut empty_kv = FDB_FAILED_ADDR;
        let mut found_sector = KvdbSecInfo::default();

        // Try using sector first
        if using_sector > 0 {
            let mut sa = self.parent.oldest_addr;
            let mut tl: u32 = 0;
            loop {
                tl += self.parent.sec_size;
                if let Ok(sec) = self.read_sector_info(sa, true) {
                    found_sector = sec;
                    if found_sector.store == SectorStoreStatus::Using
                        && found_sector.check_ok
                        && found_sector.remain > kv_size + FDB_SEC_REMAIN_THRESHOLD
                        && (found_sector.dirty == SectorDirtyStatus::False
                            || (found_sector.dirty == SectorDirtyStatus::True && !gc_request))
                    {
                        empty_kv = found_sector.empty_kv;
                        break;
                    }
                }
                let next = self.get_next_sector_addr(&found_sector, tl);
                if next == FDB_FAILED_ADDR {
                    break;
                }
                sa = next;
            }
        }

        // Try empty sector
        if empty_sector > 0 && empty_kv == FDB_FAILED_ADDR {
            if empty_sector > FDB_GC_EMPTY_SEC_THRESHOLD || gc_request {
                let mut sa = self.parent.oldest_addr;
                let mut tl: u32 = 0;
                loop {
                    tl += self.parent.sec_size;
                    if let Ok(sec) = self.read_sector_info(sa, true) {
                        found_sector = sec;
                        if found_sector.store == SectorStoreStatus::Empty
                            && found_sector.check_ok
                            && found_sector.remain > kv_size + FDB_SEC_REMAIN_THRESHOLD
                            && (found_sector.dirty == SectorDirtyStatus::False
                                || (found_sector.dirty == SectorDirtyStatus::True && !gc_request))
                        {
                            empty_kv = found_sector.empty_kv;
                            break;
                        }
                    }
                    let next = self.get_next_sector_addr(&found_sector, tl);
                    if next == FDB_FAILED_ADDR {
                        break;
                    }
                    sa = next;
                }
            } else {
                self.gc_request = true;
            }
        }

        if empty_kv == FDB_FAILED_ADDR {
            found_sector = KvdbSecInfo::default();
        }
        (empty_kv, found_sector)
    }
}

// ---------------------------------------------------------------------------
// del_kv — fdb_kvdb.c:940-1000
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn del_kv(
        &mut self,
        key: Option<&str>,
        old_kv: Option<&KvNode>,
        complete_del: bool,
    ) -> Result<(), FdbErr> {
        let owned_kv;
        let kv = match old_kv {
            Some(kv_ref) => kv_ref,
            None => match key {
                Some(key_str) => match self.find_kv(key_str) {
                    Some(found) => {
                        owned_kv = found;
                        &owned_kv
                    }
                    None => return Err(FdbErr::KvNameErr),
                },
                None => return Ok(()),
            },
        };

        if !complete_del {
            write_status(
                &mut self.parent,
                &mut self.flash,
                kv.addr.start,
                &[0xFFu8; 1],
                FDB_KV_STATUS_NUM as usize,
                3,
                false,
            )?;
            self.last_is_complete_del = true;
        } else {
            write_status(
                &mut self.parent,
                &mut self.flash,
                kv.addr.start,
                &[0xFFu8; 1],
                FDB_KV_STATUS_NUM as usize,
                4,
                true,
            )?;
            if !self.last_is_complete_del {
                if FDB_KV_USING_CACHE {
                    match key {
                        Some(key_str) => self.invalidate_kv_cache(key_str, key_str.len()),
                        None => self.invalidate_kv_cache(&kv.name, kv.name_len as usize),
                    }
                }
            }
            self.last_is_complete_del = false;
        }

        let dirty_status_addr =
            fdb_align_down(kv.addr.start, self.parent.sec_size) + SECTOR_DIRTY_OFFSET as u32;
        let current_dirty = read_status(
            &self.flash,
            dirty_status_addr,
            FDB_SECTOR_DIRTY_STATUS_NUM as usize,
        );
        if current_dirty == 1 {
            // False
            write_status(
                &mut self.parent,
                &mut self.flash,
                dirty_status_addr,
                &[0xFFu8; 1],
                FDB_SECTOR_DIRTY_STATUS_NUM as usize,
                2,
                true,
            )?;
            if FDB_KV_USING_CACHE {
                let sec_addr = fdb_align_down(kv.addr.start, self.parent.sec_size);
                if let Some(idx) = self.get_sector_cache_idx(sec_addr) {
                    self.sector_cache_table[idx].dirty = SectorDirtyStatus::True;
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// move_kv — fdb_kvdb.c:1006-1067
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn move_kv_inner(&mut self, kv: &KvNode) -> Result<(), FdbErr> {
        let kv = kv.clone();

        if kv.status == KvStatus::Write {
            self.del_kv(None, Some(&kv), false)?;
        }

        let (kv_addr, sector) = self.alloc_kv(kv.len as usize);
        if kv_addr == FDB_FAILED_ADDR {
            return Err(FdbErr::SavedFull);
        }

        if self.in_recovery_check && kv.status == KvStatus::PreDelete {
            if self.find_kv_no_cache(&kv.name).is_some() {
                self.del_kv(None, Some(&kv), true)?;
                return Ok(());
            }
        }

        self.update_sec_status(&sector, kv.len)?;

        write_status(
            &mut self.parent,
            &mut self.flash,
            kv_addr,
            &[0xFFu8; 1],
            FDB_KV_STATUS_NUM as usize,
            1,
            false,
        )?;

        let kv_len = kv.len - KV_MAGIC_OFFSET as u32;
        let mut len: u32 = 0;
        while len < kv_len {
            let size = if len + 32 < kv_len { 32 } else { kv_len - len };
            let read_sz = fdb_wg_align(size) as usize;
            let mut buf = vec![0u8; read_sz];
            self.flash
                .read(kv.addr.start + KV_MAGIC_OFFSET as u32 + len, &mut buf)?;
            self.flash.write(
                kv_addr + KV_MAGIC_OFFSET as u32 + len,
                &buf[..size as usize],
            )?;
            len += size;
        }

        write_status(
            &mut self.parent,
            &mut self.flash,
            kv_addr,
            &[0xFFu8; 1],
            FDB_KV_STATUS_NUM as usize,
            2,
            true,
        )?;

        if FDB_KV_USING_CACHE {
            self.update_sector_empty_addr_cache(
                fdb_align_down(kv_addr, self.parent.sec_size),
                kv_addr
                    + KV_HDR_DATA_SIZE
                    + fdb_wg_align(kv.name_len as u32)
                    + fdb_wg_align(kv.value_len),
            );
            self.update_kv_cache(&kv.name, kv.name_len as usize, kv_addr);
        }

        self.del_kv(None, Some(&kv), true)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// new_kv / new_kv_ex — fdb_kvdb.c:1069-1096
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn new_kv(&mut self, kv_size: usize) -> (u32, KvdbSecInfo) {
        let mut already_gc = false;
        loop {
            let (empty_kv, sector) = self.alloc_kv(kv_size);
            if empty_kv == FDB_FAILED_ADDR {
                if self.gc_request && !already_gc {
                    self.gc_collect_by_free_size(kv_size);
                    already_gc = true;
                    continue;
                } else if already_gc {
                    eprintln!(
                        "Error: Alloc an KV (size {}) failed after GC. KV full.",
                        kv_size
                    );
                    self.gc_request = false;
                }
            }
            return (empty_kv, sector);
        }
    }

    fn new_kv_ex(&mut self, key_len: usize, buf_len: usize) -> (u32, KvdbSecInfo) {
        let kv_len = KV_HDR_DATA_SIZE as usize
            + fdb_wg_align(key_len as u32) as usize
            + fdb_wg_align(buf_len as u32) as usize;
        self.new_kv(kv_len)
    }
}

// ---------------------------------------------------------------------------
// GC — fdb_kvdb.c:1098-1181 (inline iteration)
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn gc_collect_by_free_size(&mut self, free_size: usize) {
        // Count empty sectors
        let mut empty_sec_num: usize = 0;
        let mut sec_addr = self.parent.oldest_addr;
        let mut sector = KvdbSecInfo::default();
        let mut traversed_len: u32 = 0;
        loop {
            traversed_len += self.parent.sec_size;
            if let Ok(sec) = self.read_sector_info(sec_addr, false) {
                sector = sec;
                if sector.check_ok && sector.store == SectorStoreStatus::Empty {
                    empty_sec_num += 1;
                }
            }
            let next = self.get_next_sector_addr(&sector, traversed_len);
            if next == FDB_FAILED_ADDR {
                break;
            }
            sec_addr = next;
        }

        if empty_sec_num > FDB_GC_EMPTY_SEC_THRESHOLD {
            self.gc_request = false;
            return;
        }

        // Do GC — iterate all sectors looking for dirty ones
        let mut last_gc_sec_addr: u32;
        sec_addr = self.parent.oldest_addr;
        traversed_len = 0;
        sector = KvdbSecInfo::default();
        loop {
            traversed_len += self.parent.sec_size;
            if let Ok(sec) = self.read_sector_info(sec_addr, false) {
                sector = sec;
                if sector.check_ok
                    && (sector.dirty == SectorDirtyStatus::True
                        || sector.dirty == SectorDirtyStatus::GC)
                {
                    let _ = write_status(
                        &mut self.parent,
                        &mut self.flash,
                        sector.addr + SECTOR_DIRTY_OFFSET as u32,
                        &[0xFFu8; 1],
                        FDB_SECTOR_DIRTY_STATUS_NUM as usize,
                        3,
                        true,
                    );

                    let mut kv = KvNode::default();
                    let sec_for_kv = sector;
                    kv.addr.start = sector.addr + SECTOR_HDR_DATA_SIZE;
                    loop {
                        let _ = self.read_kv(&mut kv);
                        if kv.crc_is_ok
                            && (kv.status == KvStatus::Write || kv.status == KvStatus::PreDelete)
                        {
                            if self.move_kv_inner(&kv).is_err() {
                                eprintln!("Error: Moved the KV ({}) for GC failed.", kv.name);
                            }
                        }
                        let next = self.get_next_kv_addr(&sec_for_kv, &kv);
                        if next == FDB_FAILED_ADDR {
                            break;
                        }
                        kv.addr.start = next;
                    }

                    let _ = self.format_sector(sector.addr, SECTOR_NOT_COMBINED);
                    last_gc_sec_addr = sector.addr;
                    self.parent.oldest_addr = self.get_next_sector_addr(&sector, 0);

                    if let Ok(last_sec) = self.read_sector_info(last_gc_sec_addr, true) {
                        if last_sec.remain > free_size {
                            break;
                        }
                    }
                }
            }
            let next = self.get_next_sector_addr(&sector, traversed_len);
            if next == FDB_FAILED_ADDR {
                break;
            }
            sec_addr = next;
        }

        self.gc_request = false;
    }

    fn gc_collect(&mut self) {
        self.gc_collect_by_free_size(self.parent.max_size as usize);
    }
}

// ---------------------------------------------------------------------------
// create_kv_blob — fdb_kvdb.c:1184-1265
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn create_kv_blob_inner(
        &mut self,
        sector: &KvdbSecInfo,
        key: &str,
        value: &[u8],
        len: usize,
    ) -> Result<(), FdbErr> {
        if key.len() > FDB_KV_NAME_MAX {
            eprintln!("Error: The KV name length is more than {}", FDB_KV_NAME_MAX);
            return Err(FdbErr::KvNameErr);
        }

        let name_len = key.len() as u8;
        let value_len = len as u32;
        let kv_len = KV_HDR_DATA_SIZE + fdb_wg_align(name_len as u32) + fdb_wg_align(value_len);

        if kv_len > self.parent.sec_size - SECTOR_HDR_DATA_SIZE {
            eprintln!("Error: The KV size is too big");
            return Err(FdbErr::SavedFull);
        }

        let kv_addr = if sector.empty_kv != FDB_FAILED_ADDR {
            sector.empty_kv
        } else {
            let (addr, _) = self.new_kv(kv_len as usize);
            if addr == FDB_FAILED_ADDR {
                return Err(FdbErr::SavedFull);
            }
            addr
        };

        let is_full = self.update_sec_status(sector, kv_len)?;

        // Build KV header
        let mut hdr_buf = vec![0xFFu8; KV_HDR_BUF_SZ];
        write_u32_le(&mut hdr_buf, KV_MAGIC_OFFSET as usize, KV_MAGIC_WORD);
        write_u32_le(&mut hdr_buf, KV_MAGIC_OFFSET as usize + 4, kv_len);
        write_u8(&mut hdr_buf, KV_NAME_LEN_OFF, name_len);
        write_u32_le(&mut hdr_buf, KV_VALUE_LEN_OFF, value_len);

        // CRC32
        let mut crc: u32 = 0;
        let mut name_len_buf = [0xFFu8; 4];
        name_len_buf[0] = name_len;
        crc = calc_crc32(crc, &name_len_buf);
        let mut value_len_buf = [0u8; 4];
        value_len_buf[..4].copy_from_slice(&value_len.to_le_bytes());
        crc = calc_crc32(crc, &value_len_buf);
        crc = calc_crc32(crc, key.as_bytes());
        let name_align_pad = fdb_wg_align(name_len as u32) as usize - name_len as usize;
        for _ in 0..name_align_pad {
            crc = calc_crc32(crc, &[0xFF]);
        }
        crc = calc_crc32(crc, &value[..len]);
        let value_align_pad = fdb_wg_align(value_len) as usize - len;
        for _ in 0..value_align_pad {
            crc = calc_crc32(crc, &[0xFF]);
        }
        write_u32_le(&mut hdr_buf, KV_CRC_OFF, crc);

        self.write_kv_hdr(kv_addr, &hdr_buf)?;

        flash_write_align(&mut self.flash, kv_addr + KV_HDR_DATA_SIZE, key.as_bytes())?;

        if FDB_KV_USING_CACHE {
            if !is_full {
                self.update_sector_empty_addr_cache(
                    sector.addr,
                    kv_addr
                        + KV_HDR_DATA_SIZE
                        + fdb_wg_align(name_len as u32)
                        + fdb_wg_align(value_len),
                );
            }
            self.update_kv_cache(key, key.len(), kv_addr);
        }

        flash_write_align(
            &mut self.flash,
            kv_addr + KV_HDR_DATA_SIZE + fdb_wg_align(name_len as u32),
            &value[..len],
        )?;

        write_status(
            &mut self.parent,
            &mut self.flash,
            kv_addr,
            &[0xFFu8; 1],
            FDB_KV_STATUS_NUM as usize,
            2,
            true,
        )?;

        if is_full {
            self.gc_request = true;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Public set/del API — fdb_kvdb.c:1275-1378
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    pub fn kv_del(&mut self, key: &str) -> Result<(), FdbErr> {
        if !self.parent.init_ok {
            eprintln!("Error: KV ({}) isn't initialize OK.", self.parent.name);
            return Err(FdbErr::InitFailed);
        }
        self.del_kv(Some(key), None, true)
    }

    fn set_kv(&mut self, key: &str, value_buf: Option<&[u8]>) -> Result<(), FdbErr> {
        match value_buf {
            None => self.del_kv(Some(key), None, true),
            Some(buf) => {
                let (kv_addr, sector) = self.new_kv_ex(key.len(), buf.len());
                if kv_addr == FDB_FAILED_ADDR {
                    return Err(FdbErr::SavedFull);
                }
                self.cur_sector = sector;

                let kv_is_found = self.find_kv(key);
                if let Some(ref old_kv) = kv_is_found {
                    self.del_kv(Some(key), Some(old_kv), false)?;
                }
                let cur_sector = self.cur_sector;
                let result = self.create_kv_blob_inner(&cur_sector, key, buf, buf.len());
                if result.is_err() {
                    return result;
                }
                if let Some(ref old_kv) = kv_is_found {
                    self.del_kv(Some(key), Some(old_kv), true)?;
                }
                if self.gc_request {
                    self.gc_collect_by_free_size(
                        KV_HDR_DATA_SIZE as usize
                            + fdb_wg_align(key.len() as u32) as usize
                            + fdb_wg_align(buf.len() as u32) as usize,
                    );
                }
                Ok(())
            }
        }
    }

    pub fn kv_set_blob(&mut self, key: &str, blob: &Blob) -> Result<(), FdbErr> {
        if !self.parent.init_ok {
            eprintln!("Error: KV ({}) isn't initialize OK.", self.parent.name);
            return Err(FdbErr::InitFailed);
        }
        self.set_kv(key, Some(&blob.buf))
    }

    pub fn kv_set(&mut self, key: &str, value: &str) -> Result<(), FdbErr> {
        self.set_kv(key, Some(value.as_bytes()))
    }
}

// ---------------------------------------------------------------------------
// fdb_kv_set_default — fdb_kvdb.c:1386-1430
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    pub fn kv_set_default(&mut self) -> Result<(), FdbErr> {
        if FDB_KV_USING_CACHE {
            for i in 0..FDB_KV_CACHE_TABLE_SIZE {
                self.kv_cache_table[i].addr = FDB_DATA_UNUSED;
            }
        }
        let mut addr: u32 = 0;
        while addr < self.parent.max_size {
            self.format_sector(addr, SECTOR_NOT_COMBINED)?;
            addr += self.parent.sec_size;
        }
        let default_kvs = self.default_kvs.clone();
        for node in &default_kvs.kvs {
            let value_len = node.value.len();
            let sector = KvdbSecInfo {
                empty_kv: FDB_FAILED_ADDR,
                ..KvdbSecInfo::default()
            };
            let _ = self.create_kv_blob_inner(&sector, &node.key, &node.value, value_len);
        }
        self.parent.oldest_addr = 0;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// kv_load — fdb_kvdb.c:1546-1663 (inline iteration)
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    fn kv_load(&mut self) -> Result<(), FdbErr> {
        let mut check_failed_count: usize = 0;
        self.in_recovery_check = true;

        // Check all sector headers
        let mut sec_addr = self.parent.oldest_addr;
        let mut sector = KvdbSecInfo::default();
        let mut traversed_len: u32 = 0;
        loop {
            traversed_len += self.parent.sec_size;
            if let Ok(sec) = self.read_sector_info(sec_addr, false) {
                sector = sec;
                if !sector.check_ok {
                    check_failed_count += 1;
                    if !self.parent.not_formatable {
                        let _ = self.format_sector(sector.addr, SECTOR_NOT_COMBINED);
                    }
                }
            }
            let next = self.get_next_sector_addr(&sector, traversed_len);
            if next == FDB_FAILED_ADDR {
                break;
            }
            sec_addr = next;
        }

        if self.parent.not_formatable && check_failed_count > 0 {
            return Err(FdbErr::ReadErr);
        }
        if check_failed_count == self.sector_num() as usize {
            eprintln!("All sector header is incorrect. Set it to default.");
            self.kv_set_default()?;
        }

        // Check all sector header for recovery GC
        sec_addr = self.parent.oldest_addr;
        traversed_len = 0;
        sector = KvdbSecInfo::default();
        loop {
            traversed_len += self.parent.sec_size;
            if let Ok(sec) = self.read_sector_info(sec_addr, false) {
                sector = sec;
                if sector.check_ok && sector.dirty == SectorDirtyStatus::GC {
                    self.gc_request = true;
                    self.gc_collect();
                }
            }
            let next = self.get_next_sector_addr(&sector, traversed_len);
            if next == FDB_FAILED_ADDR {
                break;
            }
            sec_addr = next;
        }

        // Check all KV for recovery (inline loop, not closure)
        loop {
            let mut need_retry = false;
            let mut sa = self.parent.oldest_addr;
            let mut sec = KvdbSecInfo::default();
            let mut tl: u32 = 0;
            loop {
                tl += self.parent.sec_size;
                if let Ok(s) = self.read_sector_info(sa, false) {
                    sec = s;
                    if sec.store == SectorStoreStatus::Using || sec.store == SectorStoreStatus::Full
                    {
                        let mut kv = KvNode::default();
                        kv.addr.start = sec.addr + SECTOR_HDR_DATA_SIZE;
                        loop {
                            let _ = self.read_kv(&mut kv);
                            if kv.crc_is_ok && kv.status == KvStatus::PreDelete {
                                if self.move_kv_inner(&kv).is_err() {
                                    need_retry = true;
                                    break;
                                }
                            } else if kv.status == KvStatus::PreWrite {
                                let _ = write_status(
                                    &mut self.parent,
                                    &mut self.flash,
                                    kv.addr.start,
                                    &[0xFFu8; 1],
                                    FDB_KV_STATUS_NUM as usize,
                                    5,
                                    true,
                                );
                                need_retry = true;
                                break;
                            } else if kv.crc_is_ok && kv.status == KvStatus::Write {
                                if FDB_KV_USING_CACHE {
                                    self.update_kv_cache(
                                        &kv.name,
                                        kv.name_len as usize,
                                        kv.addr.start,
                                    );
                                }
                            }
                            let next = self.get_next_kv_addr(&sec, &kv);
                            if next == FDB_FAILED_ADDR {
                                break;
                            }
                            kv.addr.start = next;
                        }
                        if need_retry {
                            break;
                        }
                    }
                }
                let next = self.get_next_sector_addr(&sec, tl);
                if next == FDB_FAILED_ADDR {
                    break;
                }
                sa = next;
            }
            if need_retry {
                self.gc_collect();
                continue;
            }
            break;
        }

        self.in_recovery_check = false;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// fdb_kvdb_control — fdb_kvdb.c:1672-1727
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    pub fn kvdb_control(&mut self, arg: KvControlArg) -> Option<u32> {
        match arg {
            KvControlArg::SetSecSize(size) => {
                assert!(!self.parent.init_ok, "must set before init");
                self.parent.sec_size = size;
                None
            }
            KvControlArg::GetSecSize => Some(self.parent.sec_size),
            KvControlArg::SetFileMode(mode) => {
                assert!(!self.parent.init_ok, "must set before init");
                self.parent.file_mode = mode;
                None
            }
            KvControlArg::SetMaxSize(size) => {
                assert!(!self.parent.init_ok, "must set before init");
                self.parent.max_size = size;
                None
            }
            KvControlArg::SetNotFormat(not_format) => {
                assert!(!self.parent.init_ok, "must set before init");
                self.parent.not_formatable = not_format;
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------
// fdb_kvdb_init — fdb_kvdb.c:1740-1814
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    pub fn kvdb_init(
        &mut self,
        name: &str,
        path: &str,
        default_kv: Option<&DefaultKv>,
    ) -> Result<(), FdbErr> {
        assert_eq!(
            (FDB_STR_KV_VALUE_MAX_SIZE * 8) % FDB_WRITE_GRAN,
            0,
            "STR_KV_VALUE_MAX_SIZE must align with WRITE_GRAN"
        );

        let result = lowlevel::init_ex(&mut self.parent, name, path, FdbDbType::Kv);
        if result.is_err() {
            lowlevel::init_finish(&mut self.parent, result.err().unwrap());
            return result;
        }

        self.gc_request = false;
        self.in_recovery_check = false;
        if let Some(dk) = default_kv {
            self.default_kvs = dk.clone();
        } else {
            self.default_kvs = DefaultKv::default();
        }

        // Find the oldest sector address
        {
            let mut sector_oldest_addr: u32 = 0;
            let mut last_sector_status = SectorStoreStatus::Empty;
            self.parent.oldest_addr = 0;

            let mut sec_addr = self.parent.oldest_addr;
            let mut sector = KvdbSecInfo::default();
            let mut tl: u32 = 0;
            loop {
                tl += self.parent.sec_size;
                if let Ok(sec) = self.read_sector_info(sec_addr, false) {
                    sector = sec;
                    if last_sector_status == SectorStoreStatus::Empty
                        && (sector.store == SectorStoreStatus::Full
                            || sector.store == SectorStoreStatus::Using)
                    {
                        sector_oldest_addr = sector.addr;
                    }
                    last_sector_status = sector.store;
                }
                let next = self.get_next_sector_addr(&sector, tl);
                if next == FDB_FAILED_ADDR {
                    break;
                }
                sec_addr = next;
            }
            self.parent.oldest_addr = sector_oldest_addr;
        }

        assert!(
            FDB_GC_EMPTY_SEC_THRESHOLD > 0
                && FDB_GC_EMPTY_SEC_THRESHOLD < self.sector_num() as usize,
            "GC_EMPTY_SEC_THRESHOLD must be between 1 and sector_num-1"
        );

        if FDB_KV_USING_CACHE {
            for i in 0..FDB_SECTOR_CACHE_TABLE_SIZE {
                self.sector_cache_table[i].check_ok = false;
                self.sector_cache_table[i].empty_kv = FDB_FAILED_ADDR;
                self.sector_cache_table[i].addr = FDB_DATA_UNUSED;
            }
            for i in 0..FDB_KV_CACHE_TABLE_SIZE {
                self.kv_cache_table[i].addr = FDB_DATA_UNUSED;
            }
        }

        let result = self.kv_load();
        lowlevel::init_finish(
            &mut self.parent,
            if result.is_ok() {
                FdbErr::NoErr
            } else {
                result.err().unwrap()
            },
        );
        result
    }

    pub fn kvdb_deinit(&mut self) -> Result<(), FdbErr> {
        lowlevel::deinit(&mut self.parent);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Iterator — fdb_kvdb.c:1838-1896
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    pub fn kv_iterator_init(&self) -> KvIterator {
        KvIterator {
            curr_kv: KvNode {
                addr: KvNodeAddr {
                    start: 0,
                    ..KvNodeAddr::default()
                },
                ..KvNode::default()
            },
            iterated_cnt: 0,
            iterated_obj_bytes: 0,
            iterated_value_bytes: 0,
            sector_addr: self.parent.oldest_addr,
            traversed_len: 0,
        }
    }

    pub fn kv_iterate(&mut self, itr: &mut KvIterator) -> bool {
        let mut sector = KvdbSecInfo::default();
        loop {
            if self.read_sector_info(itr.sector_addr, false).is_ok() {
                sector = self
                    .read_sector_info(itr.sector_addr, false)
                    .unwrap_or_default();
                if sector.store == SectorStoreStatus::Using
                    || sector.store == SectorStoreStatus::Full
                {
                    if itr.curr_kv.addr.start == 0 {
                        itr.curr_kv.addr.start = sector.addr + SECTOR_HDR_DATA_SIZE;
                    } else {
                        let next = self.get_next_kv_addr(&sector, &itr.curr_kv);
                        if next == FDB_FAILED_ADDR {
                            // C's do-while continue jumps to while-condition which updates sector_addr.
                            // Rust's loop continue jumps to top. Must advance sector_addr here.
                            itr.curr_kv.addr.start = 0;
                            itr.traversed_len += self.parent.sec_size;
                            let next_sec = self.get_next_sector_addr(&sector, itr.traversed_len);
                            if next_sec == FDB_FAILED_ADDR {
                                return false;
                            }
                            itr.sector_addr = next_sec;
                            continue;
                        }
                        itr.curr_kv.addr.start = next;
                    }
                    loop {
                        let _ = self.read_kv(&mut itr.curr_kv);
                        if itr.curr_kv.status == KvStatus::Write && itr.curr_kv.crc_is_ok {
                            itr.iterated_cnt += 1;
                            itr.iterated_obj_bytes += itr.curr_kv.len as usize;
                            itr.iterated_value_bytes += itr.curr_kv.value_len as usize;
                            return true;
                        }
                        let next = self.get_next_kv_addr(&sector, &itr.curr_kv);
                        if next == FDB_FAILED_ADDR {
                            break;
                        }
                        itr.curr_kv.addr.start = next;
                    }
                }
            }
            itr.curr_kv.addr.start = 0;
            itr.traversed_len += self.parent.sec_size;
            let next_sec = self.get_next_sector_addr(&sector, itr.traversed_len);
            if next_sec == FDB_FAILED_ADDR {
                return false;
            }
            itr.sector_addr = next_sec;
        }
    }
}

// ---------------------------------------------------------------------------
// fdb_kvdb_check — fdb_kvdb.c:1905-1942
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    pub fn kvdb_check(&mut self) -> Result<(), FdbErr> {
        if !self.parent.init_ok {
            eprintln!("Error: KV ({}) isn't initialize OK.", self.parent.name);
            return Err(FdbErr::InitFailed);
        }

        let mut result = Ok(());
        let mut sec_addr = self.parent.oldest_addr;
        let mut traversed_len: u32 = 0;
        let mut sector = KvdbSecInfo::default();

        loop {
            traversed_len += self.parent.sec_size;
            if let Ok(sec) = self.read_sector_info(sec_addr, false) {
                sector = sec;
                if sector.store == SectorStoreStatus::Using
                    || sector.store == SectorStoreStatus::Full
                {
                    let mut kv = KvNode::default();
                    kv.addr.start = sector.addr + SECTOR_HDR_DATA_SIZE;
                    loop {
                        if self.read_kv(&mut kv).is_err() {
                            result = Err(FdbErr::ReadErr);
                            break;
                        }
                        let next = self.get_next_kv_addr(&sector, &kv);
                        if next == FDB_FAILED_ADDR {
                            break;
                        }
                        kv.addr.start = next;
                    }
                }
            }
            if result.is_err() {
                break;
            }
            sec_addr = self.get_next_sector_addr(&sector, traversed_len);
            if sec_addr == FDB_FAILED_ADDR {
                break;
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// KvDb constructor
// ---------------------------------------------------------------------------

impl<F: FlashBackend> KvDb<F> {
    pub fn new(flash: F) -> Self {
        KvDb {
            parent: FlashDb::default(),
            default_kvs: DefaultKv::default(),
            gc_request: false,
            in_recovery_check: false,
            cur_kv: KvNode::default(),
            cur_sector: KvdbSecInfo::default(),
            last_is_complete_del: false,
            kv_cache_table: [KvCacheNode::default(); FDB_KV_CACHE_TABLE_SIZE],
            sector_cache_table: [KvdbSecInfo::default(); FDB_SECTOR_CACHE_TABLE_SIZE],
            flash,
        }
    }
}

// ---------------------------------------------------------------------------
// Blob helper
// ---------------------------------------------------------------------------

pub fn kv_to_blob(kv: &KvNode, blob: &mut Blob) {
    blob.saved.meta_addr = kv.addr.start;
    blob.saved.addr = kv.addr.value;
    blob.saved.len = kv.value_len as usize;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flash::MemFlash;
    use crate::types::DefaultKvNode;

    fn make_kvdb(sec_size: u32, num_sectors: u32) -> KvDb<MemFlash> {
        let flash = MemFlash::new(sec_size, num_sectors);
        let mut db = KvDb::new(flash);
        db.parent.sec_size = sec_size;
        db.parent.max_size = sec_size * num_sectors;
        db
    }

    #[test]
    fn kvdb_init_basic() {
        let mut db = make_kvdb(4096, 4);
        let result = db.kvdb_init("test_db", "/tmp/test_kvdb", None);
        assert!(result.is_ok(), "kvdb_init failed: {:?}", result);
        assert!(db.parent.init_ok);
    }

    #[test]
    fn kvdb_set_get_string() {
        let mut db = make_kvdb(4096, 4);
        db.kvdb_init("test_db", "/tmp/test_kvdb", None).unwrap();
        db.kv_set("key1", "value1").unwrap();
        assert_eq!(db.kv_get("key1"), Some("value1".to_string()));
    }

    #[test]
    fn kvdb_set_get_blob() {
        let mut db = make_kvdb(4096, 4);
        db.kvdb_init("test_db", "/tmp/test_kvdb", None).unwrap();
        let data = vec![0x01, 0x02, 0x03, 0x04];
        let blob = Blob::make(&data);
        db.kv_set_blob("blob_key", &blob).unwrap();
        let mut read_blob = Blob::with_capacity(4);
        let read_len = db.kv_get_blob("blob_key", &mut read_blob);
        assert_eq!(read_len, 4);
        assert_eq!(&read_blob.buf[..4], &data[..]);
    }

    #[test]
    fn kvdb_delete_key() {
        let mut db = make_kvdb(4096, 4);
        db.kvdb_init("test_db", "/tmp/test_kvdb", None).unwrap();
        db.kv_set("key1", "value1").unwrap();
        assert!(db.kv_get("key1").is_some());
        db.kv_del("key1").unwrap();
        assert!(db.kv_get("key1").is_none());
    }

    #[test]
    fn kvdb_overwrite_key() {
        let mut db = make_kvdb(4096, 4);
        db.kvdb_init("test_db", "/tmp/test_kvdb", None).unwrap();
        db.kv_set("key1", "old_value").unwrap();
        db.kv_set("key1", "new_value").unwrap();
        assert_eq!(db.kv_get("key1"), Some("new_value".to_string()));
    }

    #[test]
    fn kvdb_get_nonexistent() {
        let mut db = make_kvdb(4096, 4);
        db.kvdb_init("test_db", "/tmp/test_kvdb", None).unwrap();
        assert!(db.kv_get("no_such_key").is_none());
    }

    #[test]
    fn kvdb_multiple_keys() {
        let mut db = make_kvdb(4096, 4);
        db.kvdb_init("test_db", "/tmp/test_kvdb", None).unwrap();
        db.kv_set("key1", "val1").unwrap();
        db.kv_set("key2", "val2").unwrap();
        db.kv_set("key3", "val3").unwrap();
        assert_eq!(db.kv_get("key1"), Some("val1".to_string()));
        assert_eq!(db.kv_get("key2"), Some("val2".to_string()));
        assert_eq!(db.kv_get("key3"), Some("val3".to_string()));
    }

    #[test]
    fn kvdb_set_default() {
        let mut db = make_kvdb(4096, 4);
        let default_kv = DefaultKv {
            kvs: vec![DefaultKvNode {
                key: "dk".to_string(),
                value: b"dv".to_vec(),
            }],
        };
        db.kvdb_init("test_db", "/tmp/test_kvdb", Some(&default_kv))
            .unwrap();
        db.kv_set_default().unwrap();
        let mut blob = Blob::with_capacity(32);
        let read_len = db.kv_get_blob("dk", &mut blob);
        assert!(read_len > 0);
    }

    #[test]
    fn kvdb_iterator() {
        let mut db = make_kvdb(4096, 4);
        db.kvdb_init("test_db", "/tmp/test_kvdb", None).unwrap();
        db.kv_set("key1", "val1").unwrap();
        db.kv_set("key2", "val2").unwrap();
        db.kv_set("key3", "val3").unwrap();
        let mut itr = db.kv_iterator_init();
        let mut count = 0;
        while db.kv_iterate(&mut itr) {
            count += 1;
        }
        assert_eq!(count, 3);
    }

    #[test]
    fn kvdb_check_integrity() {
        let mut db = make_kvdb(4096, 4);
        db.kvdb_init("test_db", "/tmp/test_kvdb", None).unwrap();
        db.kv_set("key1", "val1").unwrap();
        assert!(db.kvdb_check().is_ok());
    }

    #[test]
    fn kvdb_control_set_sec_size() {
        let mut db = make_kvdb(4096, 4);
        db.parent.init_ok = false;
        db.kvdb_control(KvControlArg::SetSecSize(8192));
        assert_eq!(db.parent.sec_size, 8192);
    }

    #[test]
    fn kvdb_control_get_sec_size() {
        let mut db = make_kvdb(4096, 4);
        assert_eq!(db.kvdb_control(KvControlArg::GetSecSize), Some(4096));
    }

    #[test]
    fn kvdb_deinit() {
        let mut db = make_kvdb(4096, 4);
        db.kvdb_init("test_db", "/tmp/test_kvdb", None).unwrap();
        assert!(db.parent.init_ok);
        db.kvdb_deinit().unwrap();
        assert!(!db.parent.init_ok);
    }
}
