---
name: setup
description: Configure the Claude Code status line so the ROG Slash light bar can show your remaining usage. Run once after installing the rog-slashbar plugin.
disable-model-invocation: true
---

Set up the status line for the rog-slashbar plugin. The hooks that drive the bar come with the plugin; the status line is what feeds it your usage limits, and a plugin cannot set that itself.

On every session start the plugin keeps a copy of its program at `~/.claude/slash/rog-slashbar.exe`, inside the user's home directory. That path stays the same when the plugin updates, so the status line should point there. Use the full absolute path with forward slashes only, for example `C:/Users/<name>/.claude/slash/rog-slashbar.exe`: Claude Code runs status line commands through Git Bash, which strips backslashes.

Do these steps in order:

1. **Check the program exists** at that path. If it is missing, no session has started since the plugin was installed. Tell the user to restart Claude Code and run this command again. Stop here.

2. **Read the user settings file** `~/.claude/settings.json` (create it as `{}` if it does not exist).

3. **If a `statusLine` entry already exists and does not mention `rog-slashbar`**, show it to the user and ask whether to replace it. Do not overwrite it without an explicit yes. A Claude Code session has only one status line.

4. **Otherwise add or update the `statusLine` entry**, leaving every other setting untouched:

   ```json
   "statusLine": {
     "type": "command",
     "command": "<absolute path to rog-slashbar.exe> status",
     "refreshInterval": 60
   }
   ```

5. **Check the file is still valid JSON** after the edit.

6. **Tell the user:**
   - Restart Claude Code for the status line to take effect. The bar starts by itself on the next session.
   - The bar shows remaining 5-hour usage only on claude.ai Pro, Max and Ultra plans, and only after the first reply of a session.
   - Close Armoury Crate and G-Helper, which also write to the bar.
   - Windows with an ASUS ROG laptop that has the Slash light bar is required.
   - Check the install with: `"<absolute path to rog-slashbar.exe>" ctl status`
