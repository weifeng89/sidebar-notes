# 0003: Start with a local CLI for agents

## Decision

Sidebar Notes exposes a small local CLI before adding an MCP server. It is a `cli` subcommand of the app executable, so packaging adds no second runtime. The CLI uses the same file operations and conflict rules as the UI. MCP may later be a thin adapter over these operations, with no separate storage or write policy.

## Why

The library is local, single-user, and already uses ordinary files. Codex, Cursor, Claude Code, and terminal scripts can all run commands; a CLI is easy to inspect, repeat, test, and use when the app is closed. An MCP server becomes useful when a target agent client needs discoverable typed tools or resources, but adds packaging, connection, and permission behavior before the real workflows have been validated.

## Initial contract

- Read: `list`, `search`, `show`, and `folders`.
- Write: `create`, `import`, and `save RELATIVE_PATH EXPECTED_REVISION` with content on standard input; never silently overwrite a changed file.
- `--json` gives stable machine-readable results and error codes. Human-readable output remains the default.
- Paths in command arguments are relative to the configured library root. No unrestricted filesystem operation is exposed through the agent interface.
- `import` requires an explicit source path and project folder; source-chat attribution remains unverified until the user confirms it.
- No note bodies or absolute paths go to logs. Logs may include operation IDs, stage, duration, result, error code, and library-relative paths.

## Gate for MCP

Add a local stdio MCP adapter only after two real agent workflows use the CLI successfully: find a prior prompt or artifact, and archive a new artifact with source information and later retrieve it. The adapter should offer the same operations as tools and read-only note/artifact content as resources. Do not add HTTP listening or cloud synchronization for this product.

## Open implementation gate

The CLI must be packaged within the macOS app's size target (20 MB) and operate without launching the UI. Measure the packaged size before committing to a separate binary; if it exceeds the target, reuse the app executable with CLI subcommands.
