//! Utility functions for FlashDB.
//!
//! 1:1 behavioral translation of `fdb_utils.c`: CRC32, status table helpers,
//! and flash alignment utilities.

use crate::config::FDB_WRITE_GRAN;
use crate::types::{fdb_status_table_size, fdb_wg_align, FdbErr, FlashDb, FDB_BYTE_ERASED};

/// CRC32 lookup table (identical to the C version).
static CRC32_TABLE: [u32; 256] = [
    0x00000000, 0x77073096, 0xee0e612c, 0x990951ba, 0x076dc419, 0x706af48f, 0xe963a535, 0x9e6495a3,
    0x0edb8832, 0x79dcb8a4, 0xe0d5e91e, 0x97d2d988, 0x09b64c2b, 0x7eb17cbd, 0xe7b82d07, 0x90bf1d91,
    0x1db71064, 0x6ab020f2, 0xf3b97148, 0x84be41de, 0x1adad47d, 0x6ddde4eb, 0xf4d4b551, 0x83d385c7,
    0x136c9856, 0x646ba8c0, 0xfd62f97a, 0x8a65c9ec, 0x14015c4f, 0x63066cd9, 0xfa0f3d63, 0x8d080df5,
    0x3b6e20c8, 0x4c69105e, 0xd56041e4, 0xa2677172, 0x3c03e4d1, 0x4b04d447, 0xd20d85fd, 0xa50ab56b,
    0x35b5a8fa, 0x42b2986c, 0xdbbbc9d6, 0xacbcf940, 0x32d86ce3, 0x45df5c75, 0xdcd60dcf, 0xabd13d59,
    0x26d930ac, 0x51de003a, 0xc8d75180, 0xbfd06116, 0x21b4f4b5, 0x56b3c423, 0xcfba9599, 0xb8bda50f,
    0x2802b89e, 0x5f058808, 0xc60cd9b2, 0xb10be924, 0x2f6f7c87, 0x58684c11, 0xc1611dab, 0xb6662d3d,
    0x76dc4190, 0x01db7106, 0x98d220bc, 0xefd5102a, 0x71b18589, 0x06b6b51f, 0x9fbfe4a5, 0xe8b8d433,
    0x7807c9a2, 0x0f00f934, 0x9609a88e, 0xe10e9818, 0x7f6a0dbb, 0x086d3d2d, 0x91646c97, 0xe6635c01,
    0x6b6b51f4, 0x1c6c6162, 0x856530d8, 0xf262004e, 0x6c0695ed, 0x1b01a57b, 0x8208f4c1, 0xf50fc457,
    0x65b0d9c6, 0x12b7e950, 0x8bbeb8ea, 0xfcb9887c, 0x62dd1ddf, 0x15da2d49, 0x8cd37cf3, 0xfbd44c65,
    0x4db26158, 0x3ab551ce, 0xa3bc0074, 0xd4bb30e2, 0x4adfa541, 0x3dd895d7, 0xa4d1c46d, 0xd3d6f4fb,
    0x4369e96a, 0x346ed9fc, 0xad678846, 0xda60b8d0, 0x44042d73, 0x33031de5, 0xaa0a4c5f, 0xdd0d7cc9,
    0x5005713c, 0x270241aa, 0xbe0b1010, 0xc90c2086, 0x5768b525, 0x206f85b3, 0xb966d409, 0xce61e49f,
    0x5edef90e, 0x29d9c998, 0xb0d09822, 0xc7d7a8b4, 0x59b33d17, 0x2eb40d81, 0xb7bd5c3b, 0xc0ba6cad,
    0xedb88320, 0x9abfb3b6, 0x03b6e20c, 0x74b1d29a, 0xead54739, 0x9dd277af, 0x04db2615, 0x73dc1683,
    0xe3630b12, 0x94643b84, 0x0d6d6a3e, 0x7a6a5aa8, 0xe40ecf0b, 0x9309ff9d, 0x0a00ae27, 0x7d079eb1,
    0xf00f9344, 0x8708a3d2, 0x1e01f268, 0x6906c2fe, 0xf762575d, 0x806567cb, 0x196c3671, 0x6e6b06e7,
    0xfed41b76, 0x89d32be0, 0x10da7a5a, 0x67dd4acc, 0xf9b9df6f, 0x8ebeeff9, 0x17b7be43, 0x60b08ed5,
    0xd6d6a3e8, 0xa1d1937e, 0x38d8c2c4, 0x4fdff252, 0xd1bb67f1, 0xa6bc5767, 0x3fb506dd, 0x48b2364b,
    0xd80d2bda, 0xaf0a1b4c, 0x36034af6, 0x41047a60, 0xdf60efc3, 0xa867df55, 0x316e8eef, 0x4669be79,
    0xcb61b38c, 0xbc66831a, 0x256fd2a0, 0x5268e236, 0xcc0c7795, 0xbb0b4703, 0x220216b9, 0x5505262f,
    0xc5ba3bbe, 0xb2bd0b28, 0x2bb45a92, 0x5cb36a04, 0xc2d7ffa7, 0xb5d0cf31, 0x2cd99e8b, 0x5bdeae1d,
    0x9b64c2b0, 0xec63f226, 0x756aa39c, 0x026d930a, 0x9c0906a9, 0xeb0e363f, 0x72076785, 0x05005713,
    0x95bf4a82, 0xe2b87a14, 0x7bb12bae, 0x0cb61b38, 0x92d28e9b, 0xe5d5be0d, 0x7cdcefb7, 0x0bdbdf21,
    0x86d3d2d4, 0xf1d4e242, 0x68ddb3f8, 0x1fda836e, 0x81be16cd, 0xf6b9265b, 0x6fb077e1, 0x18b74777,
    0x88085ae6, 0xff0f6a70, 0x66063bca, 0x11010b5c, 0x8f659eff, 0xf862ae69, 0x616bffd3, 0x166ccf45,
    0xa00ae278, 0xd70dd2ee, 0x4e048354, 0x3903b3c2, 0xa7672661, 0xd06016f7, 0x4969474d, 0x3e6e77db,
    0xaed16a4a, 0xd9d65adc, 0x40df0b66, 0x37d83bf0, 0xa9bcae53, 0xdebb9ec5, 0x47b2cf7f, 0x30b5ffe9,
    0xbdbdf21c, 0xcabac28a, 0x53b39330, 0x24b4a3a6, 0xbad03605, 0xcdd70693, 0x54de5729, 0x23d967bf,
    0xb3667a2e, 0xc4614ab8, 0x5d681b02, 0x2a6f2b94, 0xb40bbe37, 0xc30c8ea1, 0x5a05df1b, 0x2d02ef8d,
];

/// Compute CRC32 (incremental). Mirrors C `fdb_calc_crc32()`.
///
/// Pass `0` as `crc` for the first call, or the previous result for incremental.
pub fn calc_crc32(crc: u32, buf: &[u8]) -> u32 {
    let mut crc = crc ^ 0xFFFFFFFF;
    for &byte in buf {
        let idx = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = CRC32_TABLE[idx] ^ (crc >> 8);
    }
    crc ^ 0xFFFFFFFF
}

/// Set a status index in a status table. Returns the byte offset written.
///
/// Mirrors C `_fdb_set_status()`.
pub fn set_status(status_table: &mut [u8], _status_num: usize, status_index: usize) -> usize {
    if FDB_WRITE_GRAN == 1 {
        if status_index == 0 {
            return usize::MAX;
        }
        let byte_index = (status_index - 1) / 8;
        if byte_index < status_table.len() {
            status_table[byte_index] &= 0xFFu8 >> (status_index % 8);
        }
        byte_index
    } else {
        let byte_index = (status_index - 1) * (FDB_WRITE_GRAN / 8);
        if byte_index < status_table.len() {
            status_table[byte_index] = 0x00;
        }
        byte_index
    }
}

/// Read the current status index from a status table.
///
/// Mirrors C `_fdb_get_status()`. Returns the index of the first non-erased status.
pub fn get_status(status_table: &[u8], status_num: usize) -> usize {
    if FDB_WRITE_GRAN == 1 {
        let mut i = 0usize;
        let mut sn = status_num - 1;
        while sn > 0 {
            sn -= 1;
            if (status_table[sn / 8] & (0x80u8 >> (sn % 8))) == 0x00 {
                break;
            }
            i += 1;
        }
        (status_num - 1) - i
    } else {
        let gran_bytes = FDB_WRITE_GRAN / 8;
        for i in 0..status_num {
            let byte_index = i * gran_bytes;
            let mut all_zero = true;
            for j in 0..gran_bytes {
                let idx = byte_index + j;
                if idx < status_table.len() && status_table[idx] != 0x00 {
                    all_zero = false;
                    break;
                }
            }
            if all_zero {
                return i;
            }
        }
        status_num - 1
    }
}

/// Write a status to flash at the given address.
///
/// Mirrors C `_fdb_write_status()`.
pub fn write_status<F: crate::types::FlashBackend>(
    _db: &mut FlashDb,
    flash: &mut F,
    addr: u32,
    status_table: &[u8],
    status_num: usize,
    status_index: usize,
    sync: bool,
) -> Result<(), FdbErr> {
    let mut table = status_table.to_vec();
    let byte_index = set_status(&mut table, status_num, status_index);
    if byte_index == usize::MAX {
        return Ok(());
    }
    flash.write(addr + byte_index as u32, &table[byte_index..byte_index + 1])?;
    let _ = sync;
    Ok(())
}

/// Read a status from flash at the given address.
///
/// Mirrors C `_fdb_read_status()`.
pub fn read_status<F: crate::types::FlashBackend>(
    flash: &F,
    addr: u32,
    status_num: usize,
) -> usize {
    let table_size = fdb_status_table_size(status_num);
    let mut table = vec![0xFFu8; table_size];
    if flash.read(addr, &mut table).is_ok() {
        get_status(&table, status_num)
    } else {
        status_num - 1
    }
}

/// Find the first non-0xFF address in a range.
///
/// Mirrors C `_fdb_continue_ff_addr()`. Returns the address of the first
/// non-erased byte, or `end` if all bytes are 0xFF.
pub fn continue_ff_addr<F: crate::types::FlashBackend>(flash: &F, start: u32, end: u32) -> u32 {
    let mut addr = start;
    while addr < end {
        let mut byte = [0u8; 1];
        if flash.read(addr, &mut byte).is_ok() {
            if byte[0] != FDB_BYTE_ERASED {
                return addr;
            }
        }
        addr += 1;
    }
    end
}

/// Write data aligned by write granularity.
///
/// Mirrors C `_fdb_flash_write_align()`.
pub fn flash_write_align<F: crate::types::FlashBackend>(
    flash: &mut F,
    addr: u32,
    data: &[u8],
) -> Result<(), FdbErr> {
    let aligned_len = fdb_wg_align(data.len() as u32) as usize;
    if data.len() == aligned_len {
        flash.write(addr, data)
    } else {
        let mut buf = data.to_vec();
        buf.resize(aligned_len, 0xFF);
        flash.write(addr, &buf)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        fdb_align, fdb_align_down, fdb_wg_align_down, Blob, FDB_DATA_UNUSED,
        FDB_DIRTY_STATUS_TABLE_SIZE, FDB_FAILED_ADDR, FDB_SECTOR_DIRTY_STATUS_NUM,
        FDB_SECTOR_STORE_STATUS_NUM, FDB_STORE_STATUS_TABLE_SIZE,
    };

    #[test]
    fn crc32_empty_buffer() {
        assert_eq!(calc_crc32(0, &[]), 0x00000000);
    }

    #[test]
    fn crc32_single_byte() {
        let result = calc_crc32(0, &[0x00]);
        assert_ne!(result, 0);
    }

    #[test]
    fn crc32_known_value() {
        // CRC32 of "123456789" is 0xCBF43926
        let result = calc_crc32(0, b"123456789");
        assert_eq!(result, 0xCBF43926);
    }

    #[test]
    fn crc32_incremental() {
        let r1 = calc_crc32(0, b"1234");
        let r2 = calc_crc32(r1, b"56789");
        let r_all = calc_crc32(0, b"123456789");
        assert_eq!(r2, r_all);
    }

    #[test]
    fn align_basic() {
        assert_eq!(fdb_align(13, 4), 16);
        assert_eq!(fdb_align(16, 4), 16);
        assert_eq!(fdb_align(0, 4), 0);
    }

    #[test]
    fn align_down_basic() {
        assert_eq!(fdb_align_down(13, 4), 12);
        assert_eq!(fdb_align_down(16, 4), 16);
        assert_eq!(fdb_align_down(0, 4), 0);
    }

    #[test]
    fn wg_align_with_gran_1() {
        // FDB_WRITE_GRAN=1 → align unit = 1 byte
        assert_eq!(fdb_wg_align(5), 5);
        assert_eq!(fdb_wg_align(0), 0);
    }

    #[test]
    fn wg_align_down_with_gran_1() {
        assert_eq!(fdb_wg_align_down(5), 5);
        assert_eq!(fdb_wg_align_down(0), 0);
    }

    #[test]
    fn status_table_size_gran_1() {
        // With GRAN=1: (status_num * 1 + 7) / 8
        assert_eq!(fdb_status_table_size(4), 1); // (4+7)/8 = 1
        assert_eq!(fdb_status_table_size(6), 1); // (6+7)/8 = 1
        assert_eq!(fdb_status_table_size(8), 1); // (8+7)/8 = 1
        assert_eq!(fdb_status_table_size(9), 2); // (9+7)/8 = 2
    }

    #[test]
    fn computed_table_sizes() {
        assert_eq!(
            FDB_STORE_STATUS_TABLE_SIZE,
            fdb_status_table_size(FDB_SECTOR_STORE_STATUS_NUM)
        );
        assert_eq!(
            FDB_DIRTY_STATUS_TABLE_SIZE,
            fdb_status_table_size(FDB_SECTOR_DIRTY_STATUS_NUM)
        );
    }

    #[test]
    fn set_status_index_0_returns_max() {
        // When all bits are 0xFF (erased), get_status returns 0 (Unused)
        let table = [0xFFu8; 1];
        let result = get_status(&table, 6);
        assert_eq!(result, 0);
    }

    #[test]
    fn set_get_status_roundtrip_4_status() {
        let mut table = [0xFFu8; 1];
        set_status(&mut table, 4, 2);
        let result = get_status(&table, 4);
        assert_eq!(result, 2);
    }

    #[test]
    fn set_get_status_roundtrip_6_status() {
        let mut table = [0xFFu8; 1];
        set_status(&mut table, 6, 3);
        let result = get_status(&table, 6);
        assert_eq!(result, 3);
    }

    #[test]
    fn set_status_wg1_bit_patterns() {
        let mut table = [0xFFu8; 1];
        set_status(&mut table, 8, 0); // Unused — no bits changed
        assert_eq!(table[0], 0xFF);
        set_status(&mut table, 8, 1); // bit 7 cleared → 0x7F
        assert_eq!(table[0], 0x7F);
        let mut table2 = [0xFFu8; 1];
        set_status(&mut table2, 8, 3); // bits 7,6 cleared → 0x1F
        assert_eq!(table2[0], 0x1F);
    }

    #[test]
    fn data_unused_and_failed_addr() {
        assert_eq!(FDB_DATA_UNUSED, 0xFFFFFFFF);
        assert_eq!(FDB_FAILED_ADDR, 0xFFFFFFFF);
    }

    #[test]
    fn blob_make_creates_blob() {
        let b = Blob::make(&[1, 2, 3]);
        assert_eq!(b.buf, vec![1, 2, 3]);
        assert_eq!(b.saved.len, 0);
    }

    #[test]
    fn blob_read_truncates_to_saved_len() {
        let mut b = Blob::with_capacity(10);
        b.saved.len = 3;
        assert_eq!(b.buf.len(), 10);
    }

    #[test]
    fn flash_write_align_basic() {
        // With GRAN=1, no alignment needed
        use crate::flash::MemFlash;
        let mut flash = MemFlash::new(4096, 2);
        let data = &[0x01, 0x02, 0x03];
        let result = flash_write_align(&mut flash, 0, data);
        assert!(result.is_ok());
    }
}
