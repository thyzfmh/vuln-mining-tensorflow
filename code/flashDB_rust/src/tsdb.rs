//! Time-series database (TSDB) feature.
//!
//! 1:1 behavioral translation of `fdb_tsdb.c`.

use crate::lowlevel;
use crate::types::*;
use crate::utils::{flash_write_align, get_status, read_status, write_status};

// ---------------------------------------------------------------------------
// Helper: read/write little-endian integers from byte slices
// ---------------------------------------------------------------------------

#[inline]
fn read_u32_le(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

#[inline]
fn read_i32_le(buf: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

#[inline]
fn write_u32_le(buf: &mut [u8], off: usize, val: u32) {
    let b = val.to_le_bytes();
    buf[off..off + 4].copy_from_slice(&b);
}

#[inline]
fn write_i32_le(buf: &mut [u8], off: usize, val: i32) {
    let b = val.to_le_bytes();
    buf[off..off + 4].copy_from_slice(&b);
}

// ---------------------------------------------------------------------------
// read_tsl — fdb_tsdb.c:147-175
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    fn read_tsl(&self, tsl: &mut TslNode) -> Result<(), FdbErr> {
        let mut idx_buf = vec![0xFFu8; LOG_IDX_DATA_SIZE as usize];
        self.flash.read(tsl.addr.index, &mut idx_buf)?;

        let status_idx = get_status(&idx_buf[..TSL_STATUS_TABLE_SIZE], FDB_TSL_STATUS_NUM);
        tsl.status = match status_idx {
            0 => TslStatus::Unused,
            1 => TslStatus::PreWrite,
            2 => TslStatus::Write,
            3 => TslStatus::UserStatus1,
            4 => TslStatus::Deleted,
            5 => TslStatus::UserStatus2,
            _ => TslStatus::Unused,
        };

        if tsl.status == TslStatus::PreWrite || tsl.status == TslStatus::Unused {
            tsl.log_len = self.max_len as u32;
            tsl.addr.log = FDB_DATA_UNUSED;
            tsl.time = 0;
        } else {
            tsl.time = read_i32_le(&idx_buf, LOG_IDX_TS_OFFSET);
            tsl.log_len = read_u32_le(&idx_buf, LOG_IDX_TS_OFFSET + 4);
            tsl.addr.log = read_u32_le(&idx_buf, LOG_IDX_TS_OFFSET + 8);
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// get_next_sector_addr — fdb_tsdb.c:177-189
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    fn get_next_sector_addr(
        pre_sec: &TsdbSecInfo,
        traversed_len: u32,
        max_size: u32,
        sec_size: u32,
    ) -> u32 {
        if traversed_len + sec_size <= max_size {
            if pre_sec.addr + sec_size < max_size {
                pre_sec.addr + sec_size
            } else {
                0
            }
        } else {
            FDB_FAILED_ADDR
        }
    }

    fn get_next_tsl_addr(sector: &TsdbSecInfo, pre_tsl: &TslNode) -> u32 {
        if sector.status == SectorStoreStatus::Empty {
            return FDB_FAILED_ADDR;
        }
        if pre_tsl.addr.index + LOG_IDX_DATA_SIZE <= sector.end_idx {
            pre_tsl.addr.index + LOG_IDX_DATA_SIZE
        } else {
            FDB_FAILED_ADDR
        }
    }

    fn get_last_tsl_addr(sector: &TsdbSecInfo, pre_tsl: &TslNode) -> u32 {
        if sector.status == SectorStoreStatus::Empty {
            return FDB_FAILED_ADDR;
        }
        if pre_tsl.addr.index >= sector.addr + TSDB_SECTOR_HDR_DATA_SIZE + LOG_IDX_DATA_SIZE {
            pre_tsl.addr.index - LOG_IDX_DATA_SIZE
        } else {
            FDB_FAILED_ADDR
        }
    }

    fn get_last_sector_addr(
        pre_sec: &TsdbSecInfo,
        traversed_len: u32,
        max_size: u32,
        sec_size: u32,
    ) -> u32 {
        if traversed_len + sec_size <= max_size {
            if pre_sec.addr >= sec_size {
                pre_sec.addr - sec_size
            } else {
                max_size - sec_size
            }
        } else {
            FDB_FAILED_ADDR
        }
    }
}

// ---------------------------------------------------------------------------
// read_sector_info — fdb_tsdb.c:242-308
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    fn read_sector_info_tsdb(&mut self, addr: u32, traversal: bool) -> Result<TsdbSecInfo, FdbErr> {
        let mut sec_hdr = vec![0xFFu8; TSDB_SEC_HDR_BUF_SZ];
        if self.flash.read(addr, &mut sec_hdr).is_err() {
            return Err(FdbErr::InitFailed);
        }

        let mut sector = TsdbSecInfo::default();
        sector.addr = addr;

        sector.magic = read_u32_le(&sec_hdr, TSDB_SECTOR_MAGIC_OFFSET);
        if sector.magic != TSDB_SECTOR_MAGIC_WORD {
            return Err(FdbErr::InitFailed);
        }

        sector.check_ok = true;
        let store_status = get_status(
            &sec_hdr[..FDB_STORE_STATUS_TABLE_SIZE],
            FDB_SECTOR_STORE_STATUS_NUM,
        );
        sector.status = match store_status {
            0 => SectorStoreStatus::Unused,
            1 => SectorStoreStatus::Empty,
            2 => SectorStoreStatus::Using,
            3 => SectorStoreStatus::Full,
            _ => SectorStoreStatus::Unused,
        };

        sector.start_time = read_i32_le(&sec_hdr, TSDB_SECTOR_START_TIME_OFFSET);

        let end0_status = read_status(
            &self.flash,
            addr + TSDB_SECTOR_END0_STATUS_OFFSET as u32,
            FDB_TSL_STATUS_NUM,
        );
        let end1_status = read_status(
            &self.flash,
            addr + TSDB_SECTOR_END1_STATUS_OFFSET as u32,
            FDB_TSL_STATUS_NUM,
        );

        sector.end_info_stat[0] = match end0_status {
            0 => TslStatus::Unused,
            1 => TslStatus::PreWrite,
            2 => TslStatus::Write,
            3 => TslStatus::UserStatus1,
            4 => TslStatus::Deleted,
            5 => TslStatus::UserStatus2,
            _ => TslStatus::Unused,
        };
        sector.end_info_stat[1] = match end1_status {
            0 => TslStatus::Unused,
            1 => TslStatus::PreWrite,
            2 => TslStatus::Write,
            3 => TslStatus::UserStatus1,
            4 => TslStatus::Deleted,
            5 => TslStatus::UserStatus2,
            _ => TslStatus::Unused,
        };

        if sector.end_info_stat[0] == TslStatus::Write {
            sector.end_time = read_i32_le(&sec_hdr, TSDB_SECTOR_END0_TIME_OFFSET);
            sector.end_idx = read_u32_le(&sec_hdr, TSDB_SECTOR_END0_IDX_OFFSET);
        } else if sector.end_info_stat[1] == TslStatus::Write {
            sector.end_time = read_i32_le(&sec_hdr, TSDB_SECTOR_END1_TIME_OFFSET);
            sector.end_idx = read_u32_le(&sec_hdr, TSDB_SECTOR_END1_IDX_OFFSET);
        }

        sector.empty_idx = sector.addr + TSDB_SECTOR_HDR_DATA_SIZE;
        sector.empty_data = sector.addr + self.parent.sec_size;
        sector.remain = (sector.empty_data - sector.empty_idx) as usize;

        if sector.status == SectorStoreStatus::Using && traversal {
            let mut tsl = TslNode::default();
            tsl.addr.index = sector.empty_idx;
            loop {
                if self.read_tsl(&mut tsl).is_err() {
                    break;
                }
                if tsl.status == TslStatus::Unused {
                    break;
                }
                if tsl.status != TslStatus::PreWrite {
                    sector.end_time = tsl.time;
                }
                sector.end_idx = tsl.addr.index;
                sector.empty_idx += LOG_IDX_DATA_SIZE;
                sector.empty_data -= fdb_wg_align(tsl.log_len);
                tsl.addr.index += LOG_IDX_DATA_SIZE;
                let consumed = LOG_IDX_DATA_SIZE as usize + fdb_wg_align(tsl.log_len) as usize;
                if sector.remain > consumed {
                    sector.remain -= consumed;
                } else {
                    sector.remain = 0;
                    break;
                }
            }
        }

        Ok(sector)
    }
}

// ---------------------------------------------------------------------------
// format_sector — fdb_tsdb.c:309-328
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    fn format_sector_tsdb(&mut self, addr: u32) -> Result<(), FdbErr> {
        assert_eq!(
            addr % self.parent.sec_size,
            0,
            "addr must be sector-aligned"
        );
        self.flash.erase(addr, self.parent.sec_size as usize)?;

        write_status(
            &mut self.parent,
            &mut self.flash,
            addr,
            &[0xFFu8; 1],
            FDB_SECTOR_STORE_STATUS_NUM,
            1,
            true,
        )?;

        let mut magic_buf = [0xFFu8; 4];
        let b = TSDB_SECTOR_MAGIC_WORD.to_le_bytes();
        magic_buf[..4].copy_from_slice(&b);
        self.flash
            .write(addr + TSDB_SECTOR_MAGIC_OFFSET as u32, &magic_buf)?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// sector_iterator — fdb_tsdb.c:329-349
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    fn sector_iterator_tsdb<C>(
        &mut self,
        start_sector: &mut TsdbSecInfo,
        status_filter: SectorStoreStatus,
        traversal: bool,
        mut callback: C,
    ) where
        C: FnMut(&mut TsdbSecInfo) -> bool,
    {
        let mut sec_addr = start_sector.addr;
        let mut traversed_len: u32 = 0;

        loop {
            traversed_len += self.parent.sec_size;
            if let Ok(sec) = self.read_sector_info_tsdb(sec_addr, false) {
                *start_sector = sec;
                if status_filter == SectorStoreStatus::Unused
                    || start_sector.status == status_filter
                {
                    if traversal {
                        if let Ok(sec) = self.read_sector_info_tsdb(sec_addr, true) {
                            *start_sector = sec;
                        }
                    }
                    if callback(start_sector) {
                        return;
                    }
                }
            }
            let next = Self::get_next_sector_addr(
                start_sector,
                traversed_len,
                self.parent.max_size,
                self.parent.sec_size,
            );
            if next == FDB_FAILED_ADDR {
                return;
            }
            sec_addr = next;
        }
    }
}

// ---------------------------------------------------------------------------
// write_tsl — fdb_tsdb.c:350-378
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    fn write_tsl(&mut self, blob: &[u8], time: FdbTime) -> Result<(), FdbErr> {
        let idx_addr = self.cur_sec.empty_idx;
        let log_addr = self.cur_sec.empty_data - fdb_wg_align(blob.len() as u32);

        write_status(
            &mut self.parent,
            &mut self.flash,
            idx_addr,
            &[0xFFu8; 1],
            FDB_TSL_STATUS_NUM,
            1,
            false,
        )?;

        let mut idx_buf = vec![0xFFu8; LOG_IDX_DATA_SIZE as usize];
        write_i32_le(&mut idx_buf, LOG_IDX_TS_OFFSET, time);
        write_u32_le(&mut idx_buf, LOG_IDX_TS_OFFSET + 4, blob.len() as u32);
        write_u32_le(&mut idx_buf, LOG_IDX_TS_OFFSET + 8, log_addr);
        self.flash.write(
            idx_addr + LOG_IDX_TS_OFFSET as u32,
            &idx_buf[LOG_IDX_TS_OFFSET..LOG_IDX_DATA_SIZE as usize],
        )?;

        flash_write_align(&mut self.flash, log_addr, blob)?;

        write_status(
            &mut self.parent,
            &mut self.flash,
            idx_addr,
            &[0xFFu8; 1],
            FDB_TSL_STATUS_NUM,
            2,
            true,
        )?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// update_sec_status — fdb_tsdb.c:379-450
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    fn update_sec_status_tsdb(
        &mut self,
        sector: &mut TsdbSecInfo,
        blob_len: usize,
        cur_time: FdbTime,
    ) -> Result<(), FdbErr> {
        let aligned_blob = fdb_wg_align(blob_len as u32) as usize;

        if sector.status == SectorStoreStatus::Using
            && sector.remain < LOG_IDX_DATA_SIZE as usize + aligned_blob
        {
            let end_index_temp = sector.empty_idx - LOG_IDX_DATA_SIZE;
            let cur_sec_addr = sector.addr;

            let mut index_buf = [0xFFu8; 4];
            index_buf[..4].copy_from_slice(&end_index_temp.to_le_bytes());

            let mut time_buf = [0xFFu8; 4];
            time_buf[..4].copy_from_slice(&self.last_time.to_le_bytes());

            if sector.end_info_stat[0] == TslStatus::Unused {
                write_status(
                    &mut self.parent,
                    &mut self.flash,
                    cur_sec_addr + TSDB_SECTOR_END0_STATUS_OFFSET as u32,
                    &[0xFFu8; 1],
                    FDB_TSL_STATUS_NUM,
                    1,
                    false,
                )?;
                self.flash.write(
                    cur_sec_addr + TSDB_SECTOR_END0_TIME_OFFSET as u32,
                    &time_buf,
                )?;
                self.flash.write(
                    cur_sec_addr + TSDB_SECTOR_END0_IDX_OFFSET as u32,
                    &index_buf,
                )?;
                write_status(
                    &mut self.parent,
                    &mut self.flash,
                    cur_sec_addr + TSDB_SECTOR_END0_STATUS_OFFSET as u32,
                    &[0xFFu8; 1],
                    FDB_TSL_STATUS_NUM,
                    2,
                    true,
                )?;
            } else if sector.end_info_stat[1] == TslStatus::Unused {
                write_status(
                    &mut self.parent,
                    &mut self.flash,
                    cur_sec_addr + TSDB_SECTOR_END1_STATUS_OFFSET as u32,
                    &[0xFFu8; 1],
                    FDB_TSL_STATUS_NUM,
                    1,
                    false,
                )?;
                self.flash.write(
                    cur_sec_addr + TSDB_SECTOR_END1_TIME_OFFSET as u32,
                    &time_buf,
                )?;
                self.flash.write(
                    cur_sec_addr + TSDB_SECTOR_END1_IDX_OFFSET as u32,
                    &index_buf,
                )?;
                write_status(
                    &mut self.parent,
                    &mut self.flash,
                    cur_sec_addr + TSDB_SECTOR_END1_STATUS_OFFSET as u32,
                    &[0xFFu8; 1],
                    FDB_TSL_STATUS_NUM,
                    2,
                    true,
                )?;
            }

            write_status(
                &mut self.parent,
                &mut self.flash,
                cur_sec_addr,
                &[0xFFu8; 1],
                FDB_SECTOR_STORE_STATUS_NUM,
                3,
                true,
            )?;
            sector.status = SectorStoreStatus::Full;

            let new_sec_addr = if sector.addr + self.parent.sec_size < self.parent.max_size {
                sector.addr + self.parent.sec_size
            } else if self.rollover {
                0
            } else {
                return Err(FdbErr::SavedFull);
            };

            if let Ok(new_sec) = self.read_sector_info_tsdb(new_sec_addr, false) {
                *sector = new_sec;
            }
            if sector.status != SectorStoreStatus::Empty {
                let oldest = if new_sec_addr + self.parent.sec_size < self.parent.max_size {
                    new_sec_addr + self.parent.sec_size
                } else {
                    0
                };
                self.parent.oldest_addr = oldest;
                self.format_sector_tsdb(new_sec_addr)?;
                if let Ok(new_sec) = self.read_sector_info_tsdb(new_sec_addr, false) {
                    *sector = new_sec;
                }
            }
        } else if sector.status == SectorStoreStatus::Full {
            return Err(FdbErr::SavedFull);
        }

        if sector.status == SectorStoreStatus::Empty {
            sector.status = SectorStoreStatus::Using;
            sector.start_time = cur_time;
            write_status(
                &mut self.parent,
                &mut self.flash,
                sector.addr,
                &[0xFFu8; 1],
                FDB_SECTOR_STORE_STATUS_NUM,
                2,
                true,
            )?;

            let mut time_buf = [0xFFu8; 4];
            time_buf[..4].copy_from_slice(&cur_time.to_le_bytes());
            self.flash.write(
                sector.addr + TSDB_SECTOR_START_TIME_OFFSET as u32,
                &time_buf,
            )?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// tsl_append — fdb_tsdb.c:451-508
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    fn tsl_append_impl(&mut self, blob: &[u8], timestamp: Option<FdbTime>) -> Result<(), FdbErr> {
        let cur_time = match timestamp {
            Some(t) => t,
            None => (self.get_time.ok_or(FdbErr::WriteErr)?)(),
        };

        if blob.len() > self.max_len {
            eprintln!(
                "Warning: append length ({}) is more than max_len ({}).",
                blob.len(),
                self.max_len
            );
            return Err(FdbErr::WriteErr);
        }

        if cur_time <= self.last_time {
            eprintln!(
                "Warning: current timestamp ({}) <= last save timestamp ({}).",
                cur_time, self.last_time
            );
            return Err(FdbErr::WriteErr);
        }

        let mut cur_sec = self.cur_sec.clone();
        self.update_sec_status_tsdb(&mut cur_sec, blob.len(), cur_time)?;
        self.cur_sec = cur_sec;
        self.write_tsl(blob, cur_time)?;

        self.cur_sec.end_idx = self.cur_sec.empty_idx;
        self.cur_sec.end_time = cur_time;
        self.cur_sec.empty_idx += LOG_IDX_DATA_SIZE;
        self.cur_sec.empty_data -= fdb_wg_align(blob.len() as u32);
        self.cur_sec.remain -=
            LOG_IDX_DATA_SIZE as usize + fdb_wg_align(blob.len() as u32) as usize;
        self.last_time = cur_time;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Public API: append — fdb_tsdb.c:509-554
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    pub fn tsl_append(&mut self, blob: &[u8]) -> Result<(), FdbErr> {
        if !self.parent.init_ok {
            eprintln!("Error: TSL ({}) isn't initialize OK.", self.parent.name);
            return Err(FdbErr::InitFailed);
        }
        self.tsl_append_impl(blob, None)
    }

    pub fn tsl_append_with_ts(&mut self, blob: &[u8], timestamp: FdbTime) -> Result<(), FdbErr> {
        if !self.parent.init_ok {
            eprintln!("Error: TSL ({}) isn't initialize OK.", self.parent.name);
            return Err(FdbErr::InitFailed);
        }
        self.tsl_append_impl(blob, Some(timestamp))
    }
}

// ---------------------------------------------------------------------------
// tsl_iter — fdb_tsdb.c:556-604
// ---------------------------------------------------------------------------

pub type TslCallback<'a> = &'a mut dyn FnMut(&TslNode) -> bool;

impl<F: FlashBackend> TsDb<F> {
    pub fn tsl_iter(&mut self, cb: TslCallback) {
        if !self.parent.init_ok {
            eprintln!("Error: TSL ({}) isn't initialize OK.", self.parent.name);
            return;
        }

        let mut sec_addr = self.parent.oldest_addr;
        let mut traversed_len: u32 = 0;
        let mut sector = TsdbSecInfo::default();

        loop {
            traversed_len += self.parent.sec_size;
            if self.read_sector_info_tsdb(sec_addr, false).is_err() {
                let next = Self::get_next_sector_addr(
                    &sector,
                    traversed_len,
                    self.parent.max_size,
                    self.parent.sec_size,
                );
                if next == FDB_FAILED_ADDR {
                    return;
                }
                sec_addr = next;
                continue;
            }
            sector = self
                .read_sector_info_tsdb(sec_addr, false)
                .unwrap_or_default();
            if sector.status == SectorStoreStatus::Using || sector.status == SectorStoreStatus::Full
            {
                if sector.status == SectorStoreStatus::Using {
                    sector = self.cur_sec.clone();
                }
                let mut tsl = TslNode::default();
                tsl.addr.index = sector.addr + TSDB_SECTOR_HDR_DATA_SIZE;
                loop {
                    let _ = self.read_tsl(&mut tsl);
                    if cb(&tsl) {
                        return;
                    }
                    let next = Self::get_next_tsl_addr(&sector, &tsl);
                    if next == FDB_FAILED_ADDR {
                        break;
                    }
                    tsl.addr.index = next;
                }
            }
            let next = Self::get_next_sector_addr(
                &sector,
                traversed_len,
                self.parent.max_size,
                self.parent.sec_size,
            );
            if next == FDB_FAILED_ADDR {
                return;
            }
            sec_addr = next;
        }
    }
}

// ---------------------------------------------------------------------------
// tsl_iter_reverse — fdb_tsdb.c:606-653
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    pub fn tsl_iter_reverse(&mut self, cb: TslCallback) {
        if !self.parent.init_ok {
            eprintln!("Error: TSL ({}) isn't initialize OK.", self.parent.name);
            return;
        }

        let mut sec_addr = self.cur_sec.addr;
        let mut traversed_len: u32 = 0;
        let mut sector = TsdbSecInfo::default();

        loop {
            traversed_len += self.parent.sec_size;
            if self.read_sector_info_tsdb(sec_addr, false).is_err() {
                let next = Self::get_last_sector_addr(
                    &sector,
                    traversed_len,
                    self.parent.max_size,
                    self.parent.sec_size,
                );
                if next == FDB_FAILED_ADDR {
                    return;
                }
                sec_addr = next;
                continue;
            }
            sector = self
                .read_sector_info_tsdb(sec_addr, false)
                .unwrap_or_default();
            if sector.status == SectorStoreStatus::Using || sector.status == SectorStoreStatus::Full
            {
                if sector.status == SectorStoreStatus::Using {
                    sector = self.cur_sec.clone();
                }
                let mut tsl = TslNode::default();
                tsl.addr.index = sector.end_idx;
                loop {
                    let _ = self.read_tsl(&mut tsl);
                    if cb(&tsl) {
                        return;
                    }
                    let prev = Self::get_last_tsl_addr(&sector, &tsl);
                    if prev == FDB_FAILED_ADDR {
                        break;
                    }
                    tsl.addr.index = prev;
                }
            } else if sector.status == SectorStoreStatus::Empty
                || sector.status == SectorStoreStatus::Unused
            {
                return;
            }
            let next = Self::get_last_sector_addr(
                &sector,
                traversed_len,
                self.parent.max_size,
                self.parent.sec_size,
            );
            if next == FDB_FAILED_ADDR {
                return;
            }
            sec_addr = next;
        }
    }
}

// ---------------------------------------------------------------------------
// search_start_tsl_addr — fdb_tsdb.c:654-690
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    fn search_start_tsl_addr(&self, start: u32, end: u32, from: FdbTime, to: FdbTime) -> u32 {
        let mut s = start;
        let mut e = end;
        loop {
            let mid_idx = s + fdb_align((e - s) / 2, LOG_IDX_DATA_SIZE);
            let mut tsl = TslNode::default();
            tsl.addr.index = mid_idx;
            let _ = self.read_tsl(&mut tsl);
            if tsl.time < from {
                s = tsl.addr.index + LOG_IDX_DATA_SIZE;
            } else if tsl.time > from {
                e = tsl.addr.index - LOG_IDX_DATA_SIZE;
            } else {
                return tsl.addr.index;
            }
            if s > e {
                if from > to {
                    tsl.addr.index = s;
                    let _ = self.read_tsl(&mut tsl);
                    if tsl.time > from {
                        s -= LOG_IDX_DATA_SIZE;
                    }
                }
                break;
            }
        }
        s
    }
}

fn fdb_align(size: u32, align: u32) -> u32 {
    ((size + align - 1) / align) * align
}

// ---------------------------------------------------------------------------
// tsl_iter_by_time — fdb_tsdb.c:691-770
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    pub fn tsl_iter_by_time(&mut self, from: FdbTime, to: FdbTime, cb: TslCallback) {
        if !self.parent.init_ok {
            eprintln!("Error: TSL ({}) isn't initialize OK.", self.parent.name);
            return;
        }

        let forward = from <= to;
        let start_addr = if forward {
            self.parent.oldest_addr
        } else {
            self.cur_sec.addr
        };

        let mut sec_addr = start_addr;
        let mut traversed_len: u32 = 0;
        let mut found_start_tsl = false;
        let mut sector = TsdbSecInfo::default();

        loop {
            traversed_len += self.parent.sec_size;
            if self.read_sector_info_tsdb(sec_addr, false).is_err() {
                let next = if forward {
                    Self::get_next_sector_addr(
                        &sector,
                        traversed_len,
                        self.parent.max_size,
                        self.parent.sec_size,
                    )
                } else {
                    Self::get_last_sector_addr(
                        &sector,
                        traversed_len,
                        self.parent.max_size,
                        self.parent.sec_size,
                    )
                };
                if next == FDB_FAILED_ADDR {
                    return;
                }
                sec_addr = next;
                continue;
            }
            sector = self
                .read_sector_info_tsdb(sec_addr, false)
                .unwrap_or_default();

            if sector.status == SectorStoreStatus::Using || sector.status == SectorStoreStatus::Full
            {
                if sector.status == SectorStoreStatus::Using {
                    sector = self.cur_sec.clone();
                }

                let time_match = if forward {
                    (sec_addr == start_addr && from <= sector.start_time) || from <= sector.end_time
                } else {
                    (sec_addr == start_addr && from >= sector.end_time) || from >= sector.start_time
                };

                if found_start_tsl || (!found_start_tsl && time_match) {
                    let start = sector.addr + TSDB_SECTOR_HDR_DATA_SIZE;
                    let end = sector.end_idx;
                    found_start_tsl = true;

                    let tsl_start = self.search_start_tsl_addr(start, end, from, to);
                    let mut tsl = TslNode::default();
                    tsl.addr.index = tsl_start;

                    loop {
                        let _ = self.read_tsl(&mut tsl);
                        if tsl.status != TslStatus::Unused {
                            let in_range = if forward {
                                tsl.time >= from && tsl.time <= to
                            } else {
                                tsl.time <= from && tsl.time >= to
                            };
                            if in_range {
                                if cb(&tsl) {
                                    return;
                                }
                            } else {
                                return;
                            }
                        }
                        let next = if forward {
                            Self::get_next_tsl_addr(&sector, &tsl)
                        } else {
                            Self::get_last_tsl_addr(&sector, &tsl)
                        };
                        if next == FDB_FAILED_ADDR {
                            break;
                        }
                        tsl.addr.index = next;
                    }
                }
            } else if sector.status == SectorStoreStatus::Empty {
                return;
            }

            let next = if forward {
                Self::get_next_sector_addr(
                    &sector,
                    traversed_len,
                    self.parent.max_size,
                    self.parent.sec_size,
                )
            } else {
                Self::get_last_sector_addr(
                    &sector,
                    traversed_len,
                    self.parent.max_size,
                    self.parent.sec_size,
                )
            };
            if next == FDB_FAILED_ADDR {
                return;
            }
            sec_addr = next;
        }
    }
}

// ---------------------------------------------------------------------------
// query_count / max_blob_count — fdb_tsdb.c:771-832
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    pub fn tsl_query_count(&mut self, from: FdbTime, to: FdbTime, status: TslStatus) -> usize {
        if !self.parent.init_ok {
            eprintln!("Error: TSL ({}) isn't initialize OK.", self.parent.name);
            return 0;
        }

        let mut count = 0usize;
        let status_capture = status;
        self.tsl_iter_by_time(from, to, &mut |tsl: &TslNode| -> bool {
            if tsl.status == status_capture {
                count += 1;
            }
            false
        });
        count
    }

    pub fn tsl_max_blob_count(&self) -> usize {
        let max_blob_len = self.max_len;
        let sec_size = self.parent.sec_size as usize - TSDB_SECTOR_HDR_DATA_SIZE as usize;
        let blob_size = LOG_IDX_DATA_SIZE as usize + fdb_wg_align(max_blob_len as u32) as usize;
        let n_sec = self.parent.max_size as usize / self.parent.sec_size as usize;
        n_sec * (sec_size / blob_size)
    }
}

// ---------------------------------------------------------------------------
// tsl_set_status — fdb_tsdb.c:838-856
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    pub fn tsl_set_status(&mut self, tsl: &TslNode, status: TslStatus) -> Result<(), FdbErr> {
        let status_index = match status {
            TslStatus::Unused => 0,
            TslStatus::PreWrite => 1,
            TslStatus::Write => 2,
            TslStatus::UserStatus1 => 3,
            TslStatus::Deleted => 4,
            TslStatus::UserStatus2 => 5,
        };
        write_status(
            &mut self.parent,
            &mut self.flash,
            tsl.addr.index,
            &[0xFFu8; 1],
            FDB_TSL_STATUS_NUM,
            status_index,
            true,
        )
    }
}

// ---------------------------------------------------------------------------
// tsl_to_blob — fdb_tsdb.c:857-865
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    pub fn tsl_to_blob(&self, tsl: &TslNode, blob: &mut Blob) {
        blob.saved.addr = tsl.addr.log;
        blob.saved.meta_addr = tsl.addr.index;
        blob.saved.len = tsl.log_len as usize;
    }

    pub fn tsl_read_blob(&self, tsl: &TslNode, buf: &mut [u8]) -> Result<usize, FdbErr> {
        let len = tsl.log_len as usize;
        let read_len = if buf.len() < len { buf.len() } else { len };
        self.flash.read(tsl.addr.log, &mut buf[..read_len])?;
        Ok(read_len)
    }
}

// ---------------------------------------------------------------------------
// tsl_clean — fdb_tsdb.c:893-937
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    pub fn tsl_clean(&mut self) {
        self.tsl_format_all();
    }

    fn tsl_format_all(&mut self) {
        let mut sector = TsdbSecInfo::default();
        sector.addr = 0;
        self.sector_iterator_tsdb(&mut sector, SectorStoreStatus::Unused, false, |_sector| {
            // format each sector
            false
        });

        // Actually format all sectors
        let max_size = self.parent.max_size;
        let sec_size = self.parent.sec_size;
        let mut addr: u32 = 0;
        while addr < max_size {
            let _ = self.format_sector_tsdb(addr);
            addr += sec_size;
        }

        self.parent.oldest_addr = 0;
        self.cur_sec.addr = 0;
        self.last_time = 0;
        if let Ok(sec) = self.read_sector_info_tsdb(self.cur_sec.addr, false) {
            self.cur_sec = sec;
        }
        eprintln!("All sector format finished.");
    }
}

// ---------------------------------------------------------------------------
// TsControlArg — replaces C's int cmd + void* arg
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum TsControlArg {
    SetSecSize(u32),
    GetSecSize(*mut u32),
    SetRollover(bool),
    GetRollover(*mut bool),
    GetLastTime(*mut FdbTime),
    SetNotFormat(bool),
}

// ---------------------------------------------------------------------------
// tsdb_control — fdb_tsdb.c:938-1017
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    pub fn tsdb_control(&mut self, cmd: TsControlArg) {
        match cmd {
            TsControlArg::SetSecSize(size) => {
                assert!(!self.parent.init_ok, "sec_size must be set before init");
                self.parent.sec_size = size;
            }
            TsControlArg::GetSecSize(ptr) => unsafe {
                *ptr = self.parent.sec_size;
            },
            TsControlArg::SetRollover(val) => {
                assert!(self.parent.init_ok, "rollover must be set after init");
                self.rollover = val;
            }
            TsControlArg::GetRollover(ptr) => unsafe {
                *ptr = self.rollover;
            },
            TsControlArg::GetLastTime(ptr) => unsafe {
                *ptr = self.last_time;
            },
            TsControlArg::SetNotFormat(val) => {
                assert!(!self.parent.init_ok, "not_format must be set before init");
                self.parent.not_formatable = val;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// tsdb_init — fdb_tsdb.c:1018-1110
// ---------------------------------------------------------------------------

impl<F: FlashBackend> TsDb<F> {
    pub fn tsdb_init(
        &mut self,
        name: &str,
        path: &str,
        get_time: FdbGetTime,
        max_len: usize,
    ) -> Result<(), FdbErr> {
        let result = lowlevel::init_ex(&mut self.parent, name, path, FdbDbType::Ts);
        if result.is_err() {
            lowlevel::init_finish(&mut self.parent, result.err().unwrap());
            return result;
        }

        self.get_time = Some(get_time);
        self.max_len = max_len;
        self.rollover = true;
        self.parent.oldest_addr = FDB_DATA_UNUSED;
        self.cur_sec.addr = FDB_DATA_UNUSED;

        assert!(
            max_len < self.parent.sec_size as usize,
            "max_len must be less than sec_size"
        );

        // Check all sector headers
        let mut check_failed = false;
        let mut empty_num: usize = 0;
        let mut empty_addr: u32 = 0;
        let mut cur_sec_found = false;

        let mut sector;
        let mut sec_addr: u32 = 0;
        let mut traversed_len: u32 = 0;

        loop {
            traversed_len += self.parent.sec_size;
            match self.read_sector_info_tsdb(sec_addr, true) {
                Ok(sec) => {
                    sector = sec;
                    if !sector.check_ok {
                        check_failed = true;
                        break;
                    } else if sector.status == SectorStoreStatus::Using {
                        if !cur_sec_found {
                            self.cur_sec = sector.clone();
                            cur_sec_found = true;
                        } else {
                            check_failed = true;
                            break;
                        }
                    } else if sector.status == SectorStoreStatus::Empty {
                        empty_num += 1;
                        empty_addr = sector.addr;
                        if empty_num == 1 && !cur_sec_found {
                            self.cur_sec = sector.clone();
                            cur_sec_found = true;
                        }
                    }
                }
                Err(_) => {
                    check_failed = true;
                    break;
                }
            }
            let next = Self::get_next_sector_addr(
                &sector,
                traversed_len,
                self.parent.max_size,
                self.parent.sec_size,
            );
            if next == FDB_FAILED_ADDR {
                break;
            }
            sec_addr = next;
        }

        if check_failed {
            if self.parent.not_formatable {
                lowlevel::init_finish(&mut self.parent, FdbErr::ReadErr);
                return Err(FdbErr::ReadErr);
            } else {
                self.tsl_format_all();
            }
        } else {
            let latest_addr = if empty_num > 0 {
                empty_addr
            } else if self.rollover {
                self.cur_sec.addr
            } else {
                self.cur_sec.addr = self.parent.max_size - self.parent.sec_size;
                self.cur_sec.addr
            };

            if latest_addr + self.parent.sec_size >= self.parent.max_size {
                self.parent.oldest_addr = 0;
            } else {
                self.parent.oldest_addr = latest_addr + self.parent.sec_size;
            }
        }

        // Read current using sector info
        if let Ok(sec) = self.read_sector_info_tsdb(self.cur_sec.addr, true) {
            self.cur_sec = sec;
        }

        // Get last save time
        if self.cur_sec.status == SectorStoreStatus::Using {
            self.last_time = self.cur_sec.end_time;
        } else if self.cur_sec.status == SectorStoreStatus::Empty
            && self.parent.oldest_addr != self.cur_sec.addr
        {
            let addr = if self.cur_sec.addr == 0 {
                self.parent.max_size - self.parent.sec_size
            } else {
                self.cur_sec.addr - self.parent.sec_size
            };
            if let Ok(sec) = self.read_sector_info_tsdb(addr, false) {
                self.last_time = sec.end_time;
            }
        }

        lowlevel::init_finish(&mut self.parent, FdbErr::NoErr);
        Ok(())
    }

    pub fn tsdb_deinit(&mut self) -> Result<(), FdbErr> {
        lowlevel::deinit(&mut self.parent);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flash::MemFlash;

    fn make_tsdb(sec_size: u32, num_sectors: u32) -> TsDb<MemFlash> {
        let flash = MemFlash::new(sec_size, num_sectors);
        TsDb {
            parent: FlashDb {
                sec_size,
                max_size: sec_size * num_sectors,
                ..FlashDb::default()
            },
            cur_sec: TsdbSecInfo::default(),
            last_time: 0,
            get_time: None,
            max_len: (sec_size / 2) as usize,
            rollover: true,
            flash,
        }
    }

    thread_local! { static TIME: std::cell::Cell<i32> = std::cell::Cell::new(1); }

    fn test_get_time() -> FdbTime {
        TIME.with(|t| {
            let v = t.get();
            t.set(v + 1);
            v
        })
    }

    #[test]
    fn tsdb_init_basic() {
        let mut db = make_tsdb(4096, 4);
        db.tsdb_init("test_tsdb", "/tmp/test_tsdb", test_get_time, 512)
            .unwrap();
        assert!(db.parent.init_ok);
    }

    #[test]
    fn tsdb_deinit() {
        let mut db = make_tsdb(4096, 4);
        db.tsdb_init("test_tsdb", "/tmp/test_tsdb", test_get_time, 512)
            .unwrap();
        assert!(db.parent.init_ok);
        db.tsdb_deinit().unwrap();
        assert!(!db.parent.init_ok);
    }

    #[test]
    fn tsdb_append_and_iter() {
        let mut db = make_tsdb(4096, 4);
        db.tsdb_init("test_tsdb", "/tmp/test_tsdb", test_get_time, 512)
            .unwrap();

        let data1 = [0x01u8, 0x02, 0x03];
        let data2 = [0x04u8, 0x05, 0x06];
        let data3 = [0x07u8, 0x08, 0x09];

        db.tsl_append_with_ts(&data1, 10).unwrap();
        db.tsl_append_with_ts(&data2, 20).unwrap();
        db.tsl_append_with_ts(&data3, 30).unwrap();

        let mut count = 0;
        db.tsl_iter(&mut |_tsl: &TslNode| {
            count += 1;
            false
        });
        assert_eq!(count, 3);
    }

    #[test]
    fn tsdb_iter_by_time() {
        let mut db = make_tsdb(4096, 4);
        db.tsdb_init("test_tsdb", "/tmp/test_tsdb", test_get_time, 512)
            .unwrap();

        let data1 = [0x01u8, 0x02, 0x03];
        let data2 = [0x04u8, 0x05, 0x06];
        let data3 = [0x07u8, 0x08, 0x09];

        db.tsl_append_with_ts(&data1, 10).unwrap();
        db.tsl_append_with_ts(&data2, 20).unwrap();
        db.tsl_append_with_ts(&data3, 30).unwrap();

        let mut count = 0;
        db.tsl_iter_by_time(15, 25, &mut |tsl: &TslNode| {
            if tsl.time >= 15 && tsl.time <= 25 {
                count += 1;
            }
            false
        });
        assert_eq!(count, 1);
    }

    #[test]
    fn tsdb_query_count() {
        let mut db = make_tsdb(4096, 4);
        db.tsdb_init("test_tsdb", "/tmp/test_tsdb", test_get_time, 512)
            .unwrap();

        let data1 = [0x01u8, 0x02, 0x03];
        let data2 = [0x04u8, 0x05, 0x06];
        let data3 = [0x07u8, 0x08, 0x09];

        db.tsl_append_with_ts(&data1, 10).unwrap();
        db.tsl_append_with_ts(&data2, 20).unwrap();
        db.tsl_append_with_ts(&data3, 30).unwrap();

        let count = db.tsl_query_count(10, 30, TslStatus::Write);
        assert_eq!(count, 3);
    }

    #[test]
    fn tsdb_tsl_set_status() {
        let mut db = make_tsdb(4096, 4);
        db.tsdb_init("test_tsdb", "/tmp/test_tsdb", test_get_time, 512)
            .unwrap();

        let data = [0x01u8, 0x02, 0x03];
        db.tsl_append_with_ts(&data, 10).unwrap();

        let mut found_tsl = TslNode::default();
        db.tsl_iter(&mut |tsl: &TslNode| {
            found_tsl = tsl.clone();
            true
        });
        assert_eq!(found_tsl.status, TslStatus::Write);

        db.tsl_set_status(&found_tsl, TslStatus::Deleted).unwrap();

        let mut found_tsl2 = TslNode::default();
        db.tsl_iter(&mut |tsl: &TslNode| {
            found_tsl2 = tsl.clone();
            true
        });
        assert_eq!(found_tsl2.status, TslStatus::Deleted);
    }

    #[test]
    fn tsdb_tsl_clean() {
        let mut db = make_tsdb(4096, 4);
        db.tsdb_init("test_tsdb", "/tmp/test_tsdb", test_get_time, 512)
            .unwrap();

        let data = [0x01u8, 0x02, 0x03];
        db.tsl_append_with_ts(&data, 10).unwrap();

        db.tsl_clean();

        let mut count = 0;
        db.tsl_iter(&mut |_tsl: &TslNode| {
            count += 1;
            false
        });
        assert_eq!(count, 0);
    }

    #[test]
    fn tsdb_tsl_read_blob() {
        let mut db = make_tsdb(4096, 4);
        db.tsdb_init("test_tsdb", "/tmp/test_tsdb", test_get_time, 512)
            .unwrap();

        let data = [0xAAu8, 0xBB, 0xCC];
        db.tsl_append_with_ts(&data, 10).unwrap();

        let mut found_tsl = TslNode::default();
        db.tsl_iter(&mut |tsl: &TslNode| {
            found_tsl = tsl.clone();
            true
        });

        let mut buf = [0u8; 3];
        let read_len = db.tsl_read_blob(&found_tsl, &mut buf).unwrap();
        assert_eq!(read_len, 3);
        assert_eq!(buf, data);
    }

    #[test]
    fn tsdb_iter_reverse() {
        let mut db = make_tsdb(4096, 4);
        db.tsdb_init("test_tsdb", "/tmp/test_tsdb", test_get_time, 512)
            .unwrap();

        let data1 = [0x01u8];
        let data2 = [0x02u8];
        let data3 = [0x03u8];

        db.tsl_append_with_ts(&data1, 10).unwrap();
        db.tsl_append_with_ts(&data2, 20).unwrap();
        db.tsl_append_with_ts(&data3, 30).unwrap();

        let mut times = Vec::new();
        db.tsl_iter_reverse(&mut |tsl: &TslNode| {
            times.push(tsl.time);
            false
        });
        assert!(times.len() >= 1);
    }

    #[test]
    fn tsdb_max_blob_count() {
        let db = make_tsdb(4096, 4);
        let count = db.tsl_max_blob_count();
        assert!(count > 0);
    }

    #[test]
    fn tsdb_timestamp_must_increase() {
        let mut db = make_tsdb(4096, 4);
        db.tsdb_init("test_tsdb", "/tmp/test_tsdb", test_get_time, 512)
            .unwrap();

        let data = [0x01u8, 0x02, 0x03];
        db.tsl_append_with_ts(&data, 10).unwrap();

        let result = db.tsl_append_with_ts(&data, 5);
        assert!(result.is_err());
    }
}
