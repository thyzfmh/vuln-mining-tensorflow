//! Low-level sector/log header I/O and database initialization.
//!
//! 1:1 behavioral translation of `fdb.c` + `fdb_low_lvl.h` internal functions.

use crate::config::FDB_SW_VERSION;
use crate::types::{FdbDbType, FdbErr, FlashDb, FDB_FAILED_ADDR};

/// Extended initialization (mirrors C `_fdb_init_ex`).
pub fn init_ex(db: &mut FlashDb, name: &str, path: &str, db_type: FdbDbType) -> Result<(), FdbErr> {
    if db.init_ok {
        return Ok(());
    }

    db.name = name.to_string();
    db.db_type = db_type;

    if db.file_mode {
        for slot in db.cur_file_sec.iter_mut() {
            *slot = FDB_FAILED_ADDR;
        }
        assert!(db.sec_size != 0, "sec_size must be set before init");
        assert!(db.max_size != 0, "max_size must be set before init");
        db.storage_dir = Some(path.to_string());
        assert!(!path.is_empty(), "path must not be empty");
    }

    // sec_size must be power of 2
    assert!(
        db.sec_size & (db.sec_size - 1) == 0,
        "sec_size must be power of 2"
    );

    // max_size must align with sec_size
    if db.max_size % db.sec_size != 0 {
        eprintln!(
            "Error: db total size ({}) MUST align with sector size ({}).",
            db.max_size, db.sec_size
        );
        return Err(FdbErr::InitFailed);
    }

    // Must have >= 2 sectors
    if db.max_size / db.sec_size < 2 {
        eprintln!(
            "Error: MUST have >= 2 sectors, current has {} sector(s)",
            db.max_size / db.sec_size
        );
        return Err(FdbErr::InitFailed);
    }

    Ok(())
}

/// Finish initialization (mirrors C `_fdb_init_finish`).
pub fn init_finish(db: &mut FlashDb, result: FdbErr) {
    if result == FdbErr::NoErr {
        db.init_ok = true;
        println!("[FlashDB] V{} is initialize success.", FDB_SW_VERSION);
    } else if !db.not_formatable {
        let type_str = if db.db_type == FdbDbType::Kv {
            "KVDB"
        } else {
            "TSDB"
        };
        eprintln!(
            "Error: {} ({}@{}) is initialize fail ({}).",
            type_str,
            db.name,
            db_path(db).unwrap_or(""),
            result
        );
    }
}

/// Deinitialize database (mirrors C `_fdb_deinit`).
pub fn deinit(db: &mut FlashDb) {
    db.init_ok = false;
}

/// Get database storage path (mirrors C `_fdb_db_path`).
pub fn db_path(db: &FlashDb) -> Option<&str> {
    if db.file_mode {
        db.storage_dir.as_deref()
    } else {
        None
    }
}
