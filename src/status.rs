//! Claude Code status line command: record usage metrics, print a status line.
//! Burn rate = percent of the 5-hour limit used per minute over a rolling window.

use crate::state::{ensure_daemon, now, num, read_json, write_json, NULL};
use serde_json::{json, Value};
use std::io::Read;

const WINDOW_S: f64 = 600.0; // burn-rate window
const MIN_SPAN_S: f64 = 60.0; // history needed before a rate is computed

/// {'left','resets_at'} for a rate-limit window, or Null when unknown.
fn window(prev: &Value, rl: &Value, now: f64) -> Value {
    if let Some(used) = num(rl, "used_percentage") {
        return json!({
            "left": (100.0 - used).clamp(0.0, 100.0),
            "resets_at": rl.get("resets_at").cloned().unwrap_or(Value::Null),
        });
    }
    match num(prev, "resets_at") {
        // window reset and Claude Code dropped it -> full again
        Some(ra) if ra > 0.0 && ra <= now => json!({ "left": 100.0, "resets_at": null }),
        _ => prev.clone(),
    }
}

fn fmt(label: &str, m: &Value) -> String {
    match num(m, "left") {
        Some(l) => format!("{label}:{l:.0}%"),
        None => format!("{label}:n/a"),
    }
}

pub fn run() {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let data: Value = serde_json::from_str(&input).unwrap_or_else(|_| json!({}));
    let now = now();
    let prev = read_json("metrics.json");
    let rl = data.get("rate_limits").unwrap_or(&NULL);
    let cw = data.get("context_window").unwrap_or(&NULL);
    let get = |v: &'static str, from: &Value| from.get(v).cloned().unwrap_or(Value::Null);

    let five = window(&get("five_hour", &prev), rl.get("five_hour").unwrap_or(&NULL), now);
    let seven = window(&get("seven_day", &prev), rl.get("seven_day").unwrap_or(&NULL), now);

    let ctx_left = num(cw, "remaining_percentage").or_else(|| num(cw, "used_percentage").map(|u| 100.0 - u));
    let context = match ctx_left {
        Some(l) => json!({ "left": l.clamp(0.0, 100.0) }),
        None => get("context", &prev),
    };

    // burn rate from rolling samples [ts, used%] of 5h usage
    let mut samples: Vec<(f64, f64)> = prev
        .get("samples")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| Some((s.get(0)?.as_f64()?, s.get(1)?.as_f64()?)))
                .filter(|(ts, _)| now - ts <= WINDOW_S)
                .collect()
        })
        .unwrap_or_default();
    let mut burn = num(&prev, "burn").unwrap_or(0.0);
    if let Some(left) = num(&five, "left") {
        let used = 100.0 - left;
        if samples.last().is_some_and(|(_, u)| used < u - 1.0) {
            samples.clear(); // window reset -> start over
        }
        match samples.last_mut() {
            Some(last) if now - last.0 < 5.0 => *last = (now, used),
            _ => samples.push((now, used)),
        }
        if samples.len() > 200 {
            samples.drain(..samples.len() - 200);
        }
        let (first, last) = (samples[0], samples[samples.len() - 1]);
        let span = last.0 - first.0;
        if span >= MIN_SPAN_S {
            burn = ((last.1 - first.1) / (span / 60.0)).max(0.0);
        } else if samples.len() == 1 {
            burn = 0.0;
        }
    }

    write_json(
        "metrics.json",
        &json!({
            "ts": now, "five_hour": five, "seven_day": seven, "context": context,
            "burn": burn, "samples": samples.iter().map(|(t, u)| json!([t, u])).collect::<Vec<_>>(),
        }),
    );
    ensure_daemon();
    println!("{} {} {} burn:{burn:.1}%/m", fmt("5h", &five), fmt("7d", &seven), fmt("ctx", &context));
}
