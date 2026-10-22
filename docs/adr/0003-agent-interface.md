# 0003: Remote agents use the Mac library through a CLI

## Decision

Sidebar Notes exposes one library on Wei's Mac. Agents running on the Mac or another machine must use the same CLI operations against that Mac library. The current `cli` subcommand is the Mac-side operation layer; remote access to it is a separate, required product capability. An agent on the homelab must not create or sync its own Sidebar Notes library. MCP is not required.

## Why

The library is local to one Mac and already uses ordinary files. A remote CLI lets Hermes access that single source of truth without adding another database or a background sync protocol. The Mac-side CLI remains useful when the app UI is closed.

## Remote transport and trust boundary

Use authenticated SSH over the existing private network to invoke the packaged CLI on the Mac. Supply a stable remote client command for agents; keep all library path resolution and writes on the Mac. Restrict the SSH identity to the Sidebar Notes command surface rather than granting a general-purpose shell. Do not add an HTTP listener, cloud replica, or second writable library. Remote access must be explicitly enabled and tested from the homelab before it is called delivered.

Remote `save` sends complete Markdown through standard input and preserves the existing expected-revision conflict rule. Remote artifact import must transfer the original bytes to the Mac and then import there; a path that exists only on the homelab cannot be passed to the Mac-side `import SOURCE_PATH` command. The transfer must be bounded and validated, and failed transfers must not leave partial library files.

## Initial contract

- Read: `list`, `search`, `show`, and `folders`.
- Write: `create`, `import`, and `save RELATIVE_PATH EXPECTED_REVISION` with content on standard input; never silently overwrite a changed file.
- `--json` gives stable machine-readable results and error codes. Human-readable output remains the default.
- Paths in command arguments are relative to the configured library root. No unrestricted filesystem operation is exposed through the agent interface.
- `import` requires an explicit source path and project folder; source-chat attribution remains unverified until the user confirms it.
- No note bodies or absolute paths go to logs. Logs may include operation IDs, stage, duration, result, error code, and library-relative paths.

## Gate for MCP

No MCP adapter is planned. Revisit only if an actual agent cannot use the remote CLI.

## Open implementation gate

The Mac-side CLI is packaged and operates without launching the UI. Remote delivery additionally requires a stable client command, restricted authentication, a Mac reachability check, streaming artifact import, and end-to-end read/write/conflict tests from the homelab against an isolated Mac test library. The production library must not be used for unattended acceptance tests.
