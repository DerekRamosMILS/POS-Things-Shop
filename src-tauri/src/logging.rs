use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use log::{Level, LevelFilter, Metadata, Record};

/// Rotate the log once it grows past this size.
const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;
/// How many rotated files to keep alongside the current one.
const KEEP_ROTATIONS: usize = 3;

/// Writes to a file inside the app data directory. A GUI build on Windows has
/// no console attached, so stderr-only logging leaves nothing to diagnose with.
struct FileLogger {
    file: Mutex<Option<File>>,
    path: PathBuf,
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} [{}] {} - {}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            record.level(),
            record.target(),
            record.args()
        );

        eprint!("{}", line);

        let Ok(mut guard) = self.file.lock() else { return };
        if let Some(file) = guard.as_mut() {
            if file.metadata().map(|m| m.len() > MAX_LOG_BYTES).unwrap_or(false) {
                rotate(&self.path);
                *guard = open_log(&self.path);
            }
        }
        if let Some(file) = guard.as_mut() {
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.file.lock() {
            if let Some(file) = guard.as_mut() {
                let _ = file.flush();
            }
        }
    }
}

fn open_log(path: &Path) -> Option<File> {
    OpenOptions::new().create(true).append(true).open(path).ok()
}

fn rotate(path: &Path) {
    let base = path.to_string_lossy().to_string();
    let _ = fs::remove_file(format!("{}.{}", base, KEEP_ROTATIONS));
    for n in (1..KEEP_ROTATIONS).rev() {
        let _ = fs::rename(format!("{}.{}", base, n), format!("{}.{}", base, n + 1));
    }
    let _ = fs::rename(&base, format!("{}.1", base));
}

/// Path of the current log file, surfaced in Settings so a shop can send it in.
pub fn log_path() -> PathBuf {
    crate::db::connection::get_db_dir().join("things-shop.log")
}

pub fn init() {
    let path = log_path();
    let logger = FileLogger { file: Mutex::new(open_log(&path)), path };

    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
}
