//! Dev tool: dump rendered frames as text, consumed by docs/generate_gifs.py.
//!
//!   slash-claude frames <state> <left_pct> <burn> <seconds> [fps=14] [t0=1000]
//!
//! One line per frame: seven brightness values (0..1), left segment first.
//! Uses the real renderer, so the README animations match the actual behavior.

use crate::render::render;
use serde_json::json;

const USAGE: &str = "frames <idle|working|waiting> <left_pct> <burn> <seconds> [fps=14] [t0=1000]";

pub fn run(args: &[String]) -> Result<(), String> {
    let state = args.first().ok_or(USAGE)?.as_str();
    let num = |i: usize, default: Option<f64>| -> Result<f64, String> {
        match args.get(i) {
            Some(s) => s.parse::<f64>().map_err(|_| format!("bad number: {s}")),
            None => default.ok_or_else(|| USAGE.to_string()),
        }
    };
    let (left, burn, secs) = (num(1, None)?, num(2, None)?, num(3, None)?);
    let (fps, t0) = (num(4, Some(14.0))?, num(5, Some(1000.0))?);

    let agent = json!({ "state": state, "ts": t0 });
    let metrics = json!({ "five_hour": { "left": left }, "burn": burn });
    let cfg = json!({ "view": "five_hour" });

    for i in 0..(secs * fps).round() as usize {
        let frame = render(t0 + i as f64 / fps, &agent, &metrics, &cfg);
        let line: Vec<String> = frame.iter().map(|v| format!("{v:.4}")).collect();
        println!("{}", line.join(" "));
    }
    Ok(())
}
