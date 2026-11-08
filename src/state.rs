//! Shared state files (~/.claude/slash/*.json), logging and daemon launcher.
//! Windows only (uses Windows process-creation flags).

use serde_json::{Map, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
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

/// Stable location of this program's copy: ~/.claude/slash/rog-slashbar.exe.
/// The status line command points here, so it keeps working after a plugin update moves the
/// plugin's own directory.
pub fn stable_exe() -> PathBuf {
    state_dir().join("rog-slashbar.exe")
}

/// Copy `src` to `dir/rog-slashbar.exe` unless an identical copy is already there.
/// A running exe can be renamed but not overwritten, so an old copy is moved aside first.
pub fn sync_copy(src: &Path, dir: &Path) {
    let Ok(bytes) = fs::read(src) else { return };
    let dst = dir.join("rog-slashbar.exe");
    if fs::read(&dst).map(|d| d == bytes).unwrap_or(false) {
        return;
    }
    let _ = fs::create_dir_all(dir);
    if dst.exists() {
        let _ = fs::rename(&dst, dir.join(format!("rog-slashbar.old-{}.exe", std::process::id())));
    }
    let tmp = dir.join(format!("rog-slashbar.{}.tmp", std::process::id()));
    if fs::write(&tmp, &bytes).is_ok() && fs::rename(&tmp, &dst).is_err() {
        let _ = fs::remove_file(&tmp);
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().starts_with("rog-slashbar.old-") {
                let _ = fs::remove_file(e.path()); // still-running old copies just stay until next time
            }
        }
    }
}

/// Keep the stable copy of the running program up to date.
pub fn sync_stable_copy() {
    if let Ok(me) = std::env::current_exe() {
        if fs::canonicalize(&me).ok() != fs::canonicalize(stable_exe()).ok() {
            sync_copy(&me, &state_dir());
        }
    }
}

/// Spawn the daemon detached unless a fresh heartbeat exists.
/// The daemon enforces its own singleton lock, so racing spawns are harmless.
pub fn ensure_daemon() {
    if daemon_alive() {
        return;
    }
    let exe = if stable_exe().exists() {
        stable_exe() // run from the stable copy so a plugin update can replace its own files
    } else {
        match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return,
        }
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rog-slashbar-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn sync_copy_installs_then_skips_identical_then_replaces() {
        let dir = scratch("sync");
        let src = dir.join("src.exe");
        let dst_dir = dir.join("dst");
        fs::write(&src, b"version one").unwrap();

        sync_copy(&src, &dst_dir);
        assert_eq!(fs::read(dst_dir.join("rog-slashbar.exe")).unwrap(), b"version one");

        sync_copy(&src, &dst_dir); // identical: untouched
        assert_eq!(fs::read(dst_dir.join("rog-slashbar.exe")).unwrap(), b"version one");

        fs::write(&src, b"version two").unwrap();
        sync_copy(&src, &dst_dir); // new build: replaced, old copy cleaned up
        assert_eq!(fs::read(dst_dir.join("rog-slashbar.exe")).unwrap(), b"version two");
        let leftovers = fs::read_dir(&dst_dir).unwrap().flatten()
            .filter(|e| e.file_name().to_string_lossy().contains("old-") || e.file_name().to_string_lossy().ends_with(".tmp"))
            .count();
        assert_eq!(leftovers, 0);
        let _ = fs::remove_dir_all(&dir);
    }
}
