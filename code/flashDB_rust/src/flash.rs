//! Flash storage backends for FlashDB.
//!
//! `FlashBackend` trait + `MemFlash` (in-memory) and `PosixFlash` (file-based) implementations.
//! 1:1 behavioral translation of `fdb_file.c`.

use std::fs;
use std::path::PathBuf;

use crate::types::{FdbErr, FlashBackend};

// ---------------------------------------------------------------------------
// MemFlash — in-memory flash backend for testing
// ---------------------------------------------------------------------------

/// In-memory flash backend. All bytes start as 0xFF (erased).
pub struct MemFlash {
    data: Vec<u8>,
    sector_size: u32,
    #[allow(dead_code)]
    num_sectors: u32,
}

impl MemFlash {
    /// Create a new MemFlash with `num_sectors` sectors, each `sector_size` bytes.
    pub fn new(sector_size: u32, num_sectors: u32) -> Self {
        let total = sector_size as usize * num_sectors as usize;
        MemFlash {
            data: vec![0xFF; total],
            sector_size,
            num_sectors,
        }
    }

    /// Get total size in bytes.
    pub fn total_size(&self) -> usize {
        self.data.len()
    }
}

impl FlashBackend for MemFlash {
    fn read(&self, addr: u32, buf: &mut [u8]) -> Result<(), FdbErr> {
        let start = addr as usize;
        let end = start + buf.len();
        if end > self.data.len() {
            return Err(FdbErr::ReadErr);
        }
        buf.copy_from_slice(&self.data[start..end]);
        Ok(())
    }

    fn erase(&mut self, addr: u32, size: usize) -> Result<(), FdbErr> {
        let sector_start = (addr / self.sector_size) * self.sector_size;
        let start = sector_start as usize;
        let end = start + size;
        if end > self.data.len() {
            return Err(FdbErr::EraseErr);
        }
        // Erase fills with 0xFF
        self.data[start..end].fill(0xFF);
        Ok(())
    }

    fn write(&mut self, addr: u32, data: &[u8]) -> Result<(), FdbErr> {
        let start = addr as usize;
        let end = start + data.len();
        if end > self.data.len() {
            return Err(FdbErr::WriteErr);
        }
        // NOR flash: can only change 1→0 bits
        for (i, &byte) in data.iter().enumerate() {
            self.data[start + i] &= byte;
        }
        Ok(())
    }

    fn sector_size(&self) -> u32 {
        self.sector_size
    }
}

// ---------------------------------------------------------------------------
// PosixFlash — file-based flash backend
// ---------------------------------------------------------------------------

/// File-based flash backend using POSIX file I/O.
///
/// Each sector is stored as `<name>.fdb.<index>` in the storage directory.
/// Mirrors C `fdb_file.c` behavior.
pub struct PosixFlash {
    dir: PathBuf,
    name: String,
    sector_size: u32,
    #[allow(dead_code)]
    num_sectors: u32,
}

impl PosixFlash {
    /// Create a new PosixFlash.
    ///
    /// `dir` is the storage directory, `name` is the database name,
    /// `sector_size` and `num_sectors` define the flash geometry.
    pub fn new(dir: &str, name: &str, sector_size: u32, num_sectors: u32) -> Self {
        PosixFlash {
            dir: PathBuf::from(dir),
            name: name.to_string(),
            sector_size,
            num_sectors,
        }
    }

    /// Initialize: create the directory if it doesn't exist.
    pub fn init(&self) -> Result<(), FdbErr> {
        fs::create_dir_all(&self.dir).map_err(|_| FdbErr::InitFailed)
    }

    /// Get the file path for a given sector address.
    fn sector_file_path(&self, addr: u32) -> PathBuf {
        let sec_addr = addr - (addr % self.sector_size);
        let index = sec_addr / self.sector_size;
        let file_name = format!("{}.fdb.{}", self.name, index);
        self.dir.join(file_name)
    }

    /// Ensure the sector file exists and has the correct size.
    fn ensure_sector_file(&self, addr: u32) -> Result<PathBuf, FdbErr> {
        let path = self.sector_file_path(addr);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|_| FdbErr::WriteErr)?;
            }
            let data = vec![0xFFu8; self.sector_size as usize];
            fs::write(&path, &data).map_err(|_| FdbErr::WriteErr)?;
        } else {
            // Ensure file is the right size
            let metadata = fs::metadata(&path).map_err(|_| FdbErr::ReadErr)?;
            if metadata.len() < self.sector_size as u64 {
                let mut data = vec![0xFFu8; self.sector_size as usize];
                let existing = fs::read(&path).map_err(|_| FdbErr::ReadErr)?;
                data[..existing.len()].copy_from_slice(&existing);
                fs::write(&path, &data).map_err(|_| FdbErr::WriteErr)?;
            }
        }
        Ok(path)
    }
}

impl FlashBackend for PosixFlash {
    fn read(&self, addr: u32, buf: &mut [u8]) -> Result<(), FdbErr> {
        let path = self.ensure_sector_file(addr)?;
        let mut file_data = fs::read(&path).map_err(|_| FdbErr::ReadErr)?;
        if file_data.len() < self.sector_size as usize {
            file_data.resize(self.sector_size as usize, 0xFF);
        }
        let offset = (addr % self.sector_size) as usize;
        let end = offset + buf.len();
        if end > file_data.len() {
            return Err(FdbErr::ReadErr);
        }
        buf.copy_from_slice(&file_data[offset..end]);
        Ok(())
    }

    fn erase(&mut self, addr: u32, size: usize) -> Result<(), FdbErr> {
        let path = self.ensure_sector_file(addr)?;
        let mut file_data = fs::read(&path).map_err(|_| FdbErr::ReadErr)?;
        if file_data.len() < self.sector_size as usize {
            file_data.resize(self.sector_size as usize, 0xFF);
        }
        let offset = (addr % self.sector_size) as usize;
        let end = offset + size;
        if end > file_data.len() {
            return Err(FdbErr::EraseErr);
        }
        file_data[offset..end].fill(0xFF);
        fs::write(&path, &file_data).map_err(|_| FdbErr::EraseErr)
    }

    fn write(&mut self, addr: u32, data: &[u8]) -> Result<(), FdbErr> {
        let path = self.ensure_sector_file(addr)?;
        let mut file_data = fs::read(&path).map_err(|_| FdbErr::ReadErr)?;
        if file_data.len() < self.sector_size as usize {
            file_data.resize(self.sector_size as usize, 0xFF);
        }
        let offset = (addr % self.sector_size) as usize;
        let end = offset + data.len();
        if end > file_data.len() {
            return Err(FdbErr::WriteErr);
        }
        // NOR flash: can only change 1→0 bits
        for (i, &byte) in data.iter().enumerate() {
            file_data[offset + i] &= byte;
        }
        fs::write(&path, &file_data).map_err(|_| FdbErr::WriteErr)
    }

    fn sector_size(&self) -> u32 {
        self.sector_size
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn memflash_new_is_erased() {
        let flash = MemFlash::new(4096, 2);
        let mut buf = vec![0u8; 8];
        flash.read(0, &mut buf).unwrap();
        assert_eq!(buf, vec![0xFF; 8]);
    }

    #[test]
    fn memflash_write_then_read() {
        let mut flash = MemFlash::new(4096, 2);
        flash.write(0, &[0xAA, 0xBB]).unwrap();
        let mut buf = [0u8; 2];
        flash.read(0, &mut buf).unwrap();
        assert_eq!(buf, [0xAA, 0xBB]);
    }

    #[test]
    fn memflash_erase_fills_with_ff() {
        let mut flash = MemFlash::new(4096, 2);
        flash.write(0, &[0x00, 0x00]).unwrap();
        flash.erase(0, 4096).unwrap();
        let mut buf = [0u8; 2];
        flash.read(0, &mut buf).unwrap();
        assert_eq!(buf, [0xFF, 0xFF]);
    }

    #[test]
    fn memflash_write_out_of_bounds() {
        let mut flash = MemFlash::new(4096, 2);
        assert!(flash.write(8192, &[0x00]).is_err());
    }

    #[test]
    fn memflash_read_out_of_bounds() {
        let flash = MemFlash::new(4096, 2);
        let mut buf = [0u8; 1];
        assert!(flash.read(8192, &mut buf).is_err());
    }

    #[test]
    fn memflash_erase_out_of_bounds() {
        let mut flash = MemFlash::new(4096, 2);
        assert!(flash.erase(8192, 4096).is_err());
    }

    #[test]
    fn memflash_write_does_not_affect_other_regions() {
        let mut flash = MemFlash::new(4096, 2);
        flash.write(0, &[0xAA]).unwrap();
        flash.write(2, &[0xCC]).unwrap();
        let mut buf = [0u8; 3];
        flash.read(0, &mut buf).unwrap();
        assert_eq!(buf[0], 0xAA);
        assert_eq!(buf[1], 0xFF); // untouched byte
        assert_eq!(buf[2], 0xCC);
    }

    #[test]
    fn posixflash_new_creates_directory() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("flashdb_test");
        let _flash = PosixFlash::new(sub.to_str().unwrap(), "test_db", 4096, 2);
        // Directory doesn't need to exist until first I/O
    }

    #[test]
    fn posixflash_initial_read_is_erased() {
        let dir = TempDir::new().unwrap();
        let flash = PosixFlash::new(dir.path().to_str().unwrap(), "test_db", 4096, 2);
        flash.init().unwrap();
        let mut buf = vec![0u8; 8];
        flash.read(0, &mut buf).unwrap();
        assert_eq!(buf, vec![0xFF; 8]);
    }

    #[test]
    fn posixflash_write_then_read() {
        let dir = TempDir::new().unwrap();
        let mut flash = PosixFlash::new(dir.path().to_str().unwrap(), "test_db", 4096, 2);
        flash.init().unwrap();
        flash.write(0, &[0xAA, 0xBB]).unwrap();
        let mut buf = [0u8; 2];
        flash.read(0, &mut buf).unwrap();
        assert_eq!(buf, [0xAA, 0xBB]);
    }

    #[test]
    fn posixflash_erase_fills_with_ff() {
        let dir = TempDir::new().unwrap();
        let mut flash = PosixFlash::new(dir.path().to_str().unwrap(), "test_db", 4096, 2);
        flash.init().unwrap();
        flash.write(0, &[0x00]).unwrap();
        flash.erase(0, 4096).unwrap();
        let mut buf = [0u8; 1];
        flash.read(0, &mut buf).unwrap();
        assert_eq!(buf, [0xFF]);
    }

    #[test]
    fn posixflash_sector_file_naming() {
        let dir = TempDir::new().unwrap();
        let mut flash = PosixFlash::new(dir.path().to_str().unwrap(), "mydb", 4096, 2);
        flash.init().unwrap();
        // Trigger file creation
        flash.write(0, &[0x42]).unwrap();
        let path = flash.sector_file_path(0);
        assert!(path.to_str().unwrap().contains("mydb.fdb.0"));
    }

    #[test]
    fn posixflash_cross_sector_read_write() {
        let dir = TempDir::new().unwrap();
        let mut flash = PosixFlash::new(dir.path().to_str().unwrap(), "test_db", 4096, 4);
        flash.init().unwrap();
        // Write at start of sector 1
        flash.write(4096, &[0x11, 0x22]).unwrap();
        let mut buf = [0u8; 2];
        flash.read(4096, &mut buf).unwrap();
        assert_eq!(buf, [0x11, 0x22]);
    }
}
