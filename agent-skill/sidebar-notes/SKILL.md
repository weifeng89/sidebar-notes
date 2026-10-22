---
name: sidebar-notes
description: Use when an agent needs to find, read, create, update, or archive a Sidebar Notes prompt, note, or artifact. The canonical library is on Wei's Mac; homelab agents use the remote CLI.
---

# Sidebar Notes

Use `/home/wei/.local/bin/sidebarnotes` on the homelab; it calls the app on Wei's Mac over a restricted SSH alias. On Wei's Mac, use `'/Applications/Sidebar Notes.app/Contents/MacOS/stik' cli --json` directly. Both return JSON from the same Mac library, which is the only source of truth. Substitute the appropriate command prefix for `CLI` below.

1. Discover the target with `CLI folders`, `CLI search QUERY`, or `CLI list CATEGORY`. Use the returned library-relative path.
2. Read Markdown with `CLI show RELATIVE_PATH`. Capture its `revision` before editing.
3. Send a complete replacement through standard input: `CLI save RELATIVE_PATH REVISION < draft.md`. Exit 3 means a conflict copy was preserved; report both library-relative paths and let Wei reconcile.
4. Create a note with `CLI create 'Notes/Project'` or a folder with `CLI mkdir 'Prompt/Project'`.
5. Archive a local delivery file with `CLI import /absolute/local/file 'Docs/Project' 'Description' 'SOURCE_URL'`. A homelab client streams its bytes to the Mac. Treat source association as unconfirmed until Wei approves it.

For every write, check the JSON response and exit status. Report the returned library-relative path. Keep note bodies and absolute local paths out of progress logs. If the SSH connection fails, report that the Mac library is unavailable and retry later; no local replacement library is created.
