//! Shared state files (~/.claude/slash/*.json), logging and daemon launcher.
//! Windows only (uses Windows process-creation flags).

use serde_json::{Map, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub static NULL: Value = Value::Null;

pub fn state_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".claude").join("slash")
}

pub fn path(name: &str) -> PathBuf {
    state_dir().join(name)
}

pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Read a JSON object file; any failure yields an empty object.
pub fn read_json(name: &str) -> Value {
    fs::read_to_string(path(name))
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| Value::Object(Map::new()))
}

/// Atomic write (tmp + rename) so readers never see a half-written file.
pub fn write_json(name: &str, value: &Value) {
    let _ = fs::create_dir_all(state_dir());
    let tmp = path(&format!("{name}.{}.tmp", std::process::id()));
    if fs::write(&tmp, value.to_string()).is_err() {
        return;
    }
    for _ in 0..10 {
        if fs::rename(&tmp, path(name)).is_ok() {
            return;
        }
        sleep(Duration::from_millis(20)); // reader holds it open; retry
    }
    let _ = fs::remove_file(&tmp);
}

/// Numeric field of a JSON object.
pub fn num(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

pub fn log(msg: &str) {
    let p = path("daemon.log");
    if fs::metadata(&p).map(|m| m.len() > 100_000).unwrap_or(false) {
        let _ = fs::remove_file(&p);
    }
    let secs = now() as u64 % 86_400;
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&p) {
        let _ = writeln!(
            f,
            "{:02}:{:02}:{:02}Z {msg}",
            secs / 3600,
            secs % 3600 / 60,
            secs % 60
        );
    }
}

pub fn daemon_alive() -> bool {
    now() - num(&read_json("heartbeat"), "t").unwrap_or(0.0) < 5.0
}

/// Spawn the daemon detached unless a fresh heartbeat exists.
/// The daemon enforces its own singleton lock, so racing spawns are harmless.
pub fn ensure_daemon() {
    if daemon_alive() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW
    const BASE: u32 = 0x0000_0008 | 0x0000_0200 | 0x0800_0000;
    const BREAKAWAY: u32 = 0x0100_0000; // CREATE_BREAKAWAY_FROM_JOB
    let spawn = |flags: u32| {
        Command::new(&exe)
            .arg("daemon")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(flags)
            .spawn()
    };
    if spawn(BASE | BREAKAWAY).is_err() {
        let _ = spawn(BASE);
    }
}
