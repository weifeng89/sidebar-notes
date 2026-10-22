# Sidebar Notes acceptance record

Date: 2026-10-22. Scope: local arm64 macOS build, isolated `STIK_DEV_ROOT` test library. The source of AC1–AC12 is the supplied `38442577184542978_1789765107355_plan.md`; the later approved fork plan and bookmark design take precedence where they differ.

| Check | Result | Evidence or limit |
| --- | --- | --- |
| R1 / AC1 side panel | Partial | Native sidebar launched at the right edge; always-on-top is configured. Visibility above an arbitrary third-party app was not measured. Default is the approved three-bookmark collapsed state; click expands to 320 px. |
| AC2 expansion | Partial | 320↔720 control and animation are implemented. The approved bookmark shortcut mapping changed ⌘⇧E to sidebar toggle. Smoothness and global latency still need hands-on measurement. |
| R2 / R6 / AC3 categories and folders | Pass | Prompt, Notes, Artifacts (disk `Docs`) visible in native UI; real project directories can be created. |
| R3 / R4 / AC4 / AC5 / AC10 notes | Pass in automated tests | Creation and revision-checked auto-save write standard `.md` files. The editor uses Stik's CodeMirror. Actual 800 ms timing was not captured in native UI. |
| R5 / AC6 search | Pass in automated tests | Title and body search and Enter-to-open covered. Persistent 10,000-file index measured 411 ms cold scan and 2 ms warm search on this Mac. |
| R8 / AC7 external edits | Pass for automatic refresh | A newly written QA `.md` appeared in the native Notes list without switching categories after the `sidebar` Tauri capability was fixed. The 1-second end-to-end limit was not instrumented. |
| AC8 system Trash | Pass | Native delete removed an isolated QA note from the library, and the same file appeared in macOS `~/.Trash`. |
| R7 / AC9 shortcuts | Partial | Shortcut registration and mapping tested at code level. Cross-app keyboard use and the 300 ms latency target were not measured. |
| AC11 build gates | Pass | `./scripts/verify.sh`: frontend, Rust, Swift, format, Clippy, platform and bundle checks passed. The old instruction to remove Stik modules was superseded by the approved plan to preserve Stik features. |
| AC12 app | Partial | Native debug `.app` launched in isolation. Release `.app` build and size are recorded in the handoff after final packaging. |
| Artifact import | Pass for one real file | The supplied HTML report was imported, found through search, and displayed with source awaiting confirmation. The user explicitly deferred the other nine real-file cases. |
| Intel | Not tested | No Intel build target or Intel Mac was available. |

The local CLI supports folder listing/creation, list/search/show, note create/save, and original-format artifact import. MCP is deferred until actual agent workflows show a need beyond this CLI. Logs keep operation ID, phase, duration, result, error code, and library-relative path for 14 days without note bodies or absolute paths.
