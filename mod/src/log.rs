//! Plain text log next to the mod's save data. The log is the oracle for every in-game test.
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

static LOG: Mutex<Option<(File, Instant)>> = Mutex::new(None);

pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("LandsBetweenDungeons")
}

pub fn init() {
    let dir = data_dir();
    let _ = fs::create_dir_all(&dir);
    if let Ok(f) = OpenOptions::new().create(true).write(true).truncate(true).open(dir.join("log.txt")) {
        *LOG.lock().unwrap() = Some((f, Instant::now()));
    }
}

pub fn write(msg: &str) {
    if let Ok(mut g) = LOG.lock() {
        if let Some((f, t0)) = g.as_mut() {
            let _ = writeln!(f, "[{:9.3}] {msg}", t0.elapsed().as_secs_f64());
            let _ = f.flush();
        }
    }
}

#[macro_export]
macro_rules! log {
    ($($t:tt)*) => { $crate::log::write(&format!($($t)*)) };
}
