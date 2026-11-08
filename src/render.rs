//! Frame rendering. Pure functions of (time, state files) so they can be unit-tested.
//! Logical order: index 0 = LEFT end of the bar. Full bar = plenty left.

use crate::state::{num, NULL};
use serde_json::Value;
use std::f64::consts::PI;

pub const N: usize = 7; // segments (35 on long bars)
const HOLD_S: f64 = 8.0; // seconds each rotating view stays up
const INTRO_S: f64 = 0.7; // tick-blink announcing a view
pub const WORK_MAX_S: f64 = 180.0; // 'working' expires (Esc-interrupt fires no Stop hook)
pub const WAIT_MAX_S: f64 = 90.0; // stop blinking after this long

const VIEWS: [(&str, usize); 3] = [("five_hour", 1), ("seven_day", 2), ("context", 3)];

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum State {
    Idle,
    Working,
    Waiting,
}

fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    x.max(lo).min(hi)
}

fn unit(x: f64) -> f64 {
    clamp(x, 0.0, 1.0)
}

/// Percent remaining for a metric, treating an already-passed reset time as 100%.
pub fn left_pct(metrics: &Value, key: &str, now: f64) -> Option<f64> {
    let m = metrics.get(key).unwrap_or(&NULL);
    if let Some(reset) = num(m, "resets_at") {
        if now > reset {
            return Some(100.0);
        }
    }
    num(m, "left")
}

/// Bar fill for `left` percent: per-segment 0..1 and the fractional filled length.
fn fill(left: f64) -> ([f64; N], f64) {
    let filled = unit(left / 100.0) * N as f64;
    let mut seg = [0.0; N];
    for (i, s) in seg.iter_mut().enumerate() {
        *s = unit(filled - i as f64);
    }
    (seg, filled)
}

/// Seconds per sweep: 0 %/min -> 1.8s, ~1 %/min -> 0.7s, floor 0.4s.
pub fn sweep_period(burn: f64) -> f64 {
    clamp(1.8 / (1.0 + 1.5 * burn), 0.4, 1.8)
}

pub fn agent_state(agent: &Value, now: f64) -> (State, f64) {
    let st = agent.get("state").and_then(Value::as_str).unwrap_or("idle");
    let age = now - num(agent, "ts").unwrap_or(0.0);
    match st {
        "working" if age < WORK_MAX_S => (State::Working, age),
        "waiting" if age < WAIT_MAX_S => (State::Waiting, age),
        _ => (State::Idle, age),
    }
}

pub fn render(now: f64, agent: &Value, metrics: &Value, cfg: &Value) -> [f64; N] {
    let burn = num(metrics, "burn").unwrap_or(0.0);
    let (state, _) = agent_state(agent, now);

    let avail: Vec<(&str, usize)> = VIEWS
        .iter()
        .copied()
        .filter(|(k, _)| left_pct(metrics, k, now).is_some())
        .collect();
    if avail.is_empty() {
        let mut f = [0.0; N];
        f[0] = 0.15; // alive, but no data yet
        return f;
    }

    let pinned = cfg.get("view").and_then(Value::as_str).unwrap_or("five_hour");
    let key = if let Some(&(k, _)) = avail.iter().find(|(k, _)| *k == pinned) {
        k
    } else {
        let cycle = HOLD_S + INTRO_S;
        let (k, tick) = avail[(now / cycle).floor() as usize % avail.len()];
        let phase = now % cycle;
        if state == State::Idle && avail.len() > 1 && phase < INTRO_S {
            // dark, `tick` blink segments, dark
            let mut f = [0.0; N];
            if (0.2..0.5).contains(&phase) {
                for s in f.iter_mut().take(tick) {
                    *s = 1.0;
                }
            }
            return f;
        }
        k
    };

    let left = left_pct(metrics, key, now).unwrap_or(0.0);
    let (mut base, filled) = fill(left);
    if left < 10.0 {
        base[0] *= 0.5 + 0.5 * (now * 2.0 * PI).sin(); // low warning: first segment blinks
    }

    match state {
        State::Working => {
            let period = sweep_period(burn);
            let ph = (now % period) / period;
            let x = (1.0 - (2.0 * ph - 1.0).abs()) * (N as f64 - 1.0);
            let mut f = [0.0; N];
            for i in 0..N {
                f[i] = (base[i] * 0.35).max(unit(1.0 - (i as f64 - x).abs() / 1.3));
            }
            f
        }
        State::Waiting => {
            // needs your input: the ENTIRE bar blinks together (2 Hz)
            if (now * 4.0) as u64 % 2 == 0 {
                [1.0; N]
            } else {
                [0.0; N]
            }
        }
        State::Idle => {
            if burn > 0.05 {
                // leading edge breathes faster the higher the burn rate
                let edge = if filled < N as f64 { (filled as usize).min(N - 1) } else { N - 1 };
                base[edge] *= 0.65 + 0.35 * (2.0 * PI * now / (sweep_period(burn) * 2.0)).sin();
            }
            base
        }
    }
}

/// Verified on this laptop: data index N-1 is the LEFT end of the bar.
pub fn to_physical(frame: &[f64; N]) -> [u8; N] {
    let mut out = [0u8; N];
    for (i, v) in frame.iter().enumerate() {
        out[N - 1 - i] = (unit(*v) * 255.0) as u8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn metrics(left: f64, burn: f64) -> Value {
        json!({"five_hour": {"left": left, "resets_at": 9e9}, "burn": burn})
    }

    #[test]
    fn fill_half_bar() {
        let (seg, filled) = fill(50.0);
        assert_eq!(filled, 3.5);
        assert_eq!(seg, [1.0, 1.0, 1.0, 0.5, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn physical_order_is_reversed() {
        let mut f = [0.0; N];
        f[0] = 1.0; // left end
        let p = to_physical(&f);
        assert_eq!(p[N - 1], 255);
        assert_eq!(&p[..N - 1], &[0u8; N - 1]);
    }

    #[test]
    fn sweep_period_bounds() {
        assert_eq!(sweep_period(0.0), 1.8);
        assert_eq!(sweep_period(100.0), 0.4);
        assert!(sweep_period(1.0) < sweep_period(0.3));
    }

    #[test]
    fn working_expires() {
        let a = json!({"state": "working", "ts": 1000.0});
        assert_eq!(agent_state(&a, 1100.0).0, State::Working);
        assert_eq!(agent_state(&a, 1000.0 + WORK_MAX_S + 1.0).0, State::Idle);
        let w = json!({"state": "waiting", "ts": 0.0});
        assert_eq!(agent_state(&w, WAIT_MAX_S + 1.0).0, State::Idle);
    }

    #[test]
    fn no_data_shows_pilot_segment() {
        let f = render(100.0, &json!({}), &json!({}), &json!({}));
        assert_eq!(f[0], 0.15);
        assert!(f[1..].iter().all(|v| *v == 0.0));
    }

    #[test]
    fn idle_pinned_matches_fill() {
        let f = render(100.0, &json!({"state": "idle", "ts": 0}), &metrics(50.0, 0.0), &json!({"view": "five_hour"}));
        assert_eq!(f, [1.0, 1.0, 1.0, 0.5, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn passed_reset_means_full() {
        let m = json!({"five_hour": {"left": 5.0, "resets_at": 50.0}});
        assert_eq!(left_pct(&m, "five_hour", 100.0), Some(100.0));
        assert_eq!(left_pct(&m, "seven_day", 100.0), None);
    }

    #[test]
    fn waiting_blinks_whole_bar() {
        let a = json!({"state": "waiting", "ts": 99.0});
        let m = metrics(80.0, 0.0);
        let c = json!({});
        assert_eq!(render(100.0, &a, &m, &c), [1.0; N]); // on
        assert_eq!(render(100.3, &a, &m, &c), [0.0; N]); // off
        assert_eq!(render(100.5, &a, &m, &c), [1.0; N]); // on again
    }

    #[test]
    fn done_shows_remaining_credits_immediately() {
        let a = json!({"state": "done", "ts": 100.0});
        let m = metrics(50.0, 0.0);
        let f = render(100.1, &a, &m, &json!({}));
        assert_eq!(f, [1.0, 1.0, 1.0, 0.5, 0.0, 0.0, 0.0]); // no flash, default view = 5h
    }

    #[test]
    fn default_view_is_five_hour_not_rotating() {
        let m = json!({"five_hour": {"left": 50.0}, "seven_day": {"left": 10.0}, "context": {"left": 90.0}});
        for t in [0.0, 5.0, 12.0, 30.0, 61.0] {
            assert_eq!(render(t, &json!({}), &m, &json!({})), fill(50.0).0);
        }
    }

    #[test]
    fn working_dims_credits_with_bright_runner() {
        let a = json!({"state": "working", "ts": 100.0});
        let m = metrics(100.0, 0.0);
        let f = render(100.0, &a, &m, &json!({}));
        // credits bar (full) is dimmed to 35%; a bright runner sits somewhere on top of it
        let max = f.iter().cloned().fold(0.0, f64::max);
        let dim = f.iter().filter(|v| (**v - 0.35).abs() < 1e-9).count();
        assert!(max > 0.6, "runner should be bright, got {max}");
        assert!(dim >= N - 3, "rest of the bar should be dim, got {dim} dim segments");
    }

    #[test]
    fn working_values_in_range_and_visible() {
        let a = json!({"state": "working", "ts": 100.0});
        for t in 0..40 {
            let f = render(100.0 + t as f64 * 0.05, &a, &metrics(100.0, 1.0), &json!({"view": "five_hour"}));
            assert!(f.iter().all(|v| (0.0..=1.0).contains(v)));
            assert!(f.iter().cloned().fold(0.0, f64::max) > 0.3);
        }
    }

    #[test]
    fn rotation_announces_view_with_ticks() {
        // 2nd view (seven_day) during its intro window: 2 tick segments lit at phase 0.3
        let m = json!({"five_hour": {"left": 90.0}, "seven_day": {"left": 60.0}, "context": {"left": 70.0}});
        let cycle = HOLD_S + INTRO_S;
        let rot = json!({"view": "rotate"});
        let f = render(cycle * 1.0 + 0.3, &json!({}), &m, &rot);
        assert_eq!(f, [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        // after the intro the 7-day bar shows
        let g = render(cycle * 1.0 + 2.0, &json!({}), &m, &rot);
        assert_eq!(g, fill(60.0).0);
    }
}
