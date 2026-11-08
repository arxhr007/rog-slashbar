//! Control commands: status | view | set | stop.

use crate::state::{daemon_alive, ensure_daemon, now, read_json, write_json};
use serde_json::json;

pub const USAGE: &str = "\
slash-claude ctl status
slash-claude ctl view rotate|5h|7d|ctx      rotate every view, or pin one
slash-claude ctl set working|waiting|done|idle   fake an agent state (testing)
slash-claude ctl stop                       daemon exits and turns the bar off";

fn view_alias(s: &str) -> Option<&'static str> {
    Some(match s {
        "5h" | "five_hour" => "five_hour",
        "7d" | "seven_day" => "seven_day",
        "ctx" | "context" => "context",
        "rotate" => "rotate",
        _ => return None,
    })
}

pub fn run(args: &[String]) -> Result<(), String> {
    let cmd = args.first().map(String::as_str).unwrap_or("status");
    let arg = args.get(1).map(String::as_str).unwrap_or("");
    match cmd {
        "status" => {
            println!("daemon: {}", if daemon_alive() { "running" } else { "stopped" });
            println!("agent: {}", read_json("agent.json"));
            let mut m = read_json("metrics.json");
            if let Some(o) = m.as_object_mut() {
                o.remove("samples");
            }
            println!("metrics: {m}");
            println!("config: {}", read_json("config.json"));
        }
        "view" => {
            let v = view_alias(arg).ok_or("view: rotate | 5h | 7d | ctx")?;
            write_json("config.json", &json!({ "view": v }));
            println!("view -> {v}");
        }
        "set" => {
            let st = if arg.is_empty() { "idle" } else { arg };
            write_json("agent.json", &json!({ "state": st, "ts": now() }));
            ensure_daemon();
            println!("agent -> {st}");
        }
        "stop" => {
            write_json("agent.json", &json!({ "state": "ended", "ts": now() - 10.0 }));
            println!("stopping daemon");
        }
        _ => return Err(USAGE.to_string()),
    }
    Ok(())
}
