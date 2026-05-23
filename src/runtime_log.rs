use std::fs::{OpenOptions, create_dir_all};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static LOG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn debug(message: impl AsRef<str>) {
    write_log("debug", message.as_ref());
}

pub fn info(message: impl AsRef<str>) {
    write_log("info", message.as_ref());
}

pub fn warn(message: impl AsRef<str>) {
    write_log("warn", message.as_ref());
}

pub fn error(message: impl AsRef<str>) {
    write_log("error", message.as_ref());
}

fn write_log(level: &str, message: &str) {
    let lock = LOG_LOCK.get_or_init(|| Mutex::new(()));
    let _guard = lock.lock();
    let _ = create_dir_all("logs");
    let path = Path::new("logs").join("runtime.log");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "[{level}] {message}");
    }
}
