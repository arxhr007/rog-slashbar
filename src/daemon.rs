//! The daemon: sole owner of the HID device. Renders at ~15 fps from the state files.

use crate::bar::Bar;
use crate::render::{render, to_physical};
use crate::state::{log, now, num, path, read_json, state_dir, write_json};
use serde_json::json;
use std::fs::{self, OpenOptions, TryLockError};
use std::thread::sleep;
use std::time::{Duration, Instant};

const FRAME: Duration = Duration::from_micros(66_667); // ~15 fps
const IDLE_EXIT_S: f64 = 20.0 * 60.0; // exit if no hook/status updates for this long

pub fn run() {
    let _ = fs::create_dir_all(state_dir());
    // LockFileEx needs read or write access, so not append-only.
    let lock = match OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path("daemon.lock")) {
        Ok(f) => f,
        Err(e) => {
            log(&format!("cannot open lock file: {e}"));
            return;
        }
    };
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return, // another daemon already running
        Err(TryLockError::Error(e)) => {
            log(&format!("lock failed: {e}"));
            return;
        }
    }
    log("daemon start");

    let mut bar = Bar::new();
    let start = now();
    let (mut agent, mut metrics, mut cfg) = (json!({}), json!({}), json!({}));
    let (mut last_read, mut last_hb, mut last_try) = (0.0_f64, 0.0_f64, 0.0_f64);

    loop {
        let tick = Instant::now();
        let t = now();
        if t - last_read >= 0.5 {
            agent = read_json("agent.json");
            metrics = read_json("metrics.json");
            cfg = read_json("config.json");
            last_read = t;
        }
        if t - last_hb >= 1.0 {
            write_json("heartbeat", &json!({ "t": t }));
            last_hb = t;
        }
        let agent_ts = num(&agent, "ts").unwrap_or(0.0);
        if agent.get("state").and_then(|s| s.as_str()) == Some("ended") && t - agent_ts > 3.0 {
            break;
        }
        let newest = agent_ts.max(num(&metrics, "ts").unwrap_or(0.0)).max(start);
        if t - newest > IDLE_EXIT_S {
            break;
        }
        if !bar.ok() && t - last_try > 5.0 {
            bar.connect();
            last_try = t;
        }
        if bar.ok() {
            bar.show(to_physical(&render(t, &agent, &metrics, &cfg)));
        }
        sleep(FRAME.saturating_sub(tick.elapsed()));
    }

    bar.off();
    let _ = fs::remove_file(path("heartbeat"));
    log("daemon exit");
    drop(lock);
}
