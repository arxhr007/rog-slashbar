//! Claude Code hook: record agent state. Silent; the caller always exits 0.
//!
//! Beyond the state named on the command line, the hook payload (stdin JSON) refines it:
//!  - PreToolUse for AskUserQuestion / ExitPlanMode  -> "waiting" (Claude is about to ask you)
//!  - Notification of type idle_prompt / auth_success -> ignored (not a request for input)

use crate::state::{ensure_daemon, now, write_json};
use serde_json::{json, Value};
use std::io::Read;

/// Tools that stop and wait for the user's answer.
const ASKING_TOOLS: [&str; 2] = ["AskUserQuestion", "ExitPlanMode"];

fn str_field<'a>(payload: &'a Value, key: &str) -> &'a str {
    payload.get(key).and_then(Value::as_str).unwrap_or("")
}

/// Decide the state to record, or None to leave the current state alone.
pub fn decide(arg: &str, payload: &Value) -> Option<&'static str> {
    let event = str_field(payload, "hook_event_name");
    match arg {
        "start" => Some("idle"),
        "ended" => Some("ended"),
        "done" => Some("done"),
        "working" => {
            if event == "PreToolUse" && ASKING_TOOLS.contains(&str_field(payload, "tool_name")) {
                Some("waiting")
            } else {
                Some("working")
            }
        }
        "waiting" => {
            let kind = str_field(payload, "notification_type");
            let msg = str_field(payload, "message");
            let idle_nudge = kind == "idle_prompt"
                || kind == "auth_success"
                || (kind.is_empty() && msg.contains("waiting for your input"));
            if idle_nudge {
                None
            } else {
                Some("waiting")
            }
        }
        _ => Some("idle"),
    }
}

pub fn run(args: &[String]) {
    let arg = args.first().map(String::as_str).unwrap_or("idle");
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input); // always drain the hook payload
    let payload: Value = serde_json::from_str(&input).unwrap_or(Value::Null);

    let Some(state) = decide(arg, &payload) else { return };
    write_json("agent.json", &json!({ "state": state, "ts": now() }));
    if state != "ended" {
        ensure_daemon();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_states_pass_through() {
        assert_eq!(decide("working", &json!({"hook_event_name": "UserPromptSubmit"})), Some("working"));
        assert_eq!(decide("done", &Value::Null), Some("done"));
        assert_eq!(decide("start", &Value::Null), Some("idle"));
        assert_eq!(decide("ended", &Value::Null), Some("ended"));
    }

    #[test]
    fn asking_tools_mean_waiting_only_before_the_tool() {
        let pre = json!({"hook_event_name": "PreToolUse", "tool_name": "AskUserQuestion"});
        let post = json!({"hook_event_name": "PostToolUse", "tool_name": "AskUserQuestion"});
        assert_eq!(decide("working", &pre), Some("waiting"));
        assert_eq!(decide("working", &post), Some("working")); // answered -> running again
        let bash = json!({"hook_event_name": "PreToolUse", "tool_name": "Bash"});
        assert_eq!(decide("working", &bash), Some("working"));
        let plan = json!({"hook_event_name": "PreToolUse", "tool_name": "ExitPlanMode"});
        assert_eq!(decide("working", &plan), Some("waiting"));
    }

    #[test]
    fn idle_nudges_do_not_blink() {
        let idle = json!({"hook_event_name": "Notification", "notification_type": "idle_prompt"});
        assert_eq!(decide("waiting", &idle), None);
        let old = json!({"hook_event_name": "Notification", "message": "Claude is waiting for your input"});
        assert_eq!(decide("waiting", &old), None);
        let perm = json!({"hook_event_name": "Notification", "notification_type": "permission_prompt",
                          "message": "Claude needs your permission to use Bash"});
        assert_eq!(decide("waiting", &perm), Some("waiting"));
        assert_eq!(decide("waiting", &Value::Null), Some("waiting")); // unknown payload -> be safe, blink
    }
}
