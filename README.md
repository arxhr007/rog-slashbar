# slash-claude

Use the ASUS ROG **Slash** light bar (Zephyrus G14 2024+) as a live Claude Code dashboard.

| Situation | Bar |
|---|---|
| Idle / prompt finished | Remaining 5-hour credits, filled left to right |
| Prompt running | Credits bar dimmed, with a bright segment running over it (faster at a higher burn rate) |
| Claude needs your input (permission prompt, option question, plan approval) | The whole bar blinks |

Weekly and context-window bars are available with `slash-claude ctl view 7d|ctx|rotate`.

## How it works

One Windows binary with four subcommands:

- `daemon` is the only process that talks to the HID device and renders at about 15 fps. It is started automatically.
- `hook <state>` is called from Claude Code hooks and records the agent state.
- `status` is the Claude Code status line command. It reads `rate_limits` and `context_window` from stdin, records usage and burn rate, and prints a line.
- `ctl status|view|set|stop` controls the daemon.

State lives in `~/.claude/slash/*.json`. The hook and status commands only write files, so nothing fights over the HID device.

The status line data (`rate_limits`) is only present for claude.ai Pro, Max and Ultra plans, and only after the first API response of a session.

## Build

```
cargo build --release
```

Copy `target\release\slash-claude.exe` somewhere permanent (a running exe cannot be overwritten, but it can be renamed).

## Configure Claude Code

Add to `~/.claude/settings.json`. Use forward slashes: Claude Code runs commands through Git Bash, which strips backslashes.

```json
{
  "statusLine": { "type": "command", "command": "C:/path/to/slash-claude.exe status", "refreshInterval": 60 },
  "hooks": {
    "SessionStart":     [{ "hooks": [{ "type": "command", "command": "C:/path/to/slash-claude.exe hook start",   "timeout": 5 }] }],
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "C:/path/to/slash-claude.exe hook working", "timeout": 5 }] }],
    "PreToolUse":       [{ "hooks": [{ "type": "command", "command": "C:/path/to/slash-claude.exe hook working", "timeout": 5 }] }],
    "PostToolUse":      [{ "hooks": [{ "type": "command", "command": "C:/path/to/slash-claude.exe hook working", "timeout": 5 }] }],
    "Notification":     [{ "hooks": [{ "type": "command", "command": "C:/path/to/slash-claude.exe hook waiting", "timeout": 5 }] }],
    "Stop":             [{ "hooks": [{ "type": "command", "command": "C:/path/to/slash-claude.exe hook done",    "timeout": 5 }] }],
    "SessionEnd":       [{ "hooks": [{ "type": "command", "command": "C:/path/to/slash-claude.exe hook ended",   "timeout": 5 }] }]
  }
}
```

Restart Claude Code afterwards; hooks are loaded at session start.

## Notes and limits

- Windows only. Close Armoury Crate and G-Helper, which also write to the bar.
- Tested on a device that shows up as `0B05:19B6` (report ID `0x5D`, usage page `0xFF31`). `0B05:193B` is also tried.
- The bar protocol is unofficial and reverse-engineered; the byte sequences follow [G-Helper](https://github.com/seerge/g-helper)'s `SlashDevice.cs`. A firmware update could change it.
- Pressing Esc in Claude Code fires no hook, so "running" expires on its own after 3 minutes.
- Several simultaneous Claude sessions share one state.
- Tunables (timeouts, speeds, segment count) are constants in `src/`; changing them needs a rebuild.

## Tests

```
cargo test
```
