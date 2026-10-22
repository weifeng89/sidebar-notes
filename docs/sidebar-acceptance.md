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
| R7 / AC9 shortcuts | Partial | Shortcut registration and mapping tested at code level. The isolated `STIK_DEV_ROOT` profile deliberately clears all global shortcuts to avoid taking over the user's keys, so cross-app keyboard use and the 300 ms latency target were not measured. Production registration results are logged without shortcut text. |
| AC11 build gates | Pass | `./scripts/verify.sh`: frontend, Rust, Swift, format, Clippy, platform and bundle checks passed. The old instruction to remove Stik modules was superseded by the approved plan to preserve Stik features. |
| AC12 app | Partial | Native debug `.app` launched in isolation. Release `.app` build and size are recorded in the handoff after final packaging. |
| Artifact import | Pass for one real file | The supplied HTML report was imported, found through search, and displayed with source awaiting confirmation. The user explicitly deferred the other nine real-file cases. |
| Intel | Out of scope | Approved target is Apple Silicon M-series only. |

The Mac-side CLI supports folder listing/creation, list/search/show, note create/save, and original-format artifact import. A remote SSH client and restricted Mac `remote` command use that same library, including streamed artifact bytes. An isolated actual homelab-to-Mac run passed create, show, save, preserved conflict, import, and search. The dedicated restricted forced-command key rejected an attempted `whoami` with a remote protocol error, and the installed release app answered a homelab search with JSON. With that same key temporarily pointed at an isolated `STIK_DEV_ROOT` debug build, an actual homelab run passed create, show, save, preserved conflict copy, original-format artifact import, and search. The forced command was then restored to the installed release app and its original `authorized_keys` entry was checked byte-for-byte. No acceptance write used the personal library. MCP is not planned. Logs keep operation ID, phase, duration, result, error code, and library-relative path for 14 days without note bodies or absolute paths.

At revision `e11269e`, `./scripts/verify.sh` passed in 57 seconds with an isolated Bun 1.4.1 binary and a temporary Swift wrapper adding `--disable-sandbox` for the execution environment. It reported 284 Vitest passes, 164 Rust unit passes (2 ignored) plus 8 Rust integration passes, and 43 Swift passes. The repository verification script and the user's global Bun installation were not modified.

## Release closeout — 2026-10-23

Built and installed the Apple Silicon release bundle from the final working tree at `src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Sidebar Notes.app` and `/Applications/Sidebar Notes.app`. Bundle size after system `strip` and ad-hoc re-signing: 13,872 KiB. `codesign --verify --deep --strict` passed. It is not Developer ID signed or notarized; this is a local installable build.

| Release check | Result | Evidence or limit |
| --- | --- | --- |
| Product identity | Pass | Installed bundle identifier is `com.weifeng89.sidebarnotes`, display name and webview metadata are Sidebar Notes. |
| Library initialization | Pass | Confirmed `~/Documents/SideNotes/Prompt`, `Notes`, and `Docs`. |
| Bookmark UI and sizing | Pass | Installed app showed Prompt / Notes / Artifacts bookmarks; collapsed bookmark state rendered; expanded content was 320 CSS px wide at 2x display scale. |
| Esc collapse | Pass | Expanded sidebar collapsed to the three bookmarks. |
| ⌘⇧E and ⌘⇧N | Unverified | Registration log reports both shortcuts registered successfully. CUA key injection did not activate either action, so real keyboard delivery remains unverified. |
| New note persistence | Pass via UI button | Created a Markdown note and verified a zero-byte `.md` appeared immediately under `Notes/`; removed it through the app and confirmed it was in `~/.Trash`. The shortcut-specific path remains unverified above. |
| External file watcher | Pass | Created a uniquely named Markdown file externally; the UI listed it and changed status to “external file synced.” |
| Delete to Trash | Pass | Removed the QA file through the UI; source path disappeared and matching file appeared in `~/.Trash`. Both QA files remain recoverable in Trash. |
| Full repository gate | Pass | `./scripts/verify.sh`: 284 Vitest, 164 Rust unit (2 ignored), 8 Rust integration, and 43 Swift tests passed; formatting, Clippy, platform, and bundle checks passed. |

Release build warnings: the Rust toolchain's `rust-objcopy` could not load `libLLVM.dylib`, so Apple's `/usr/bin/strip` was used followed by ad-hoc re-signing. The build also skipped notarization because signing credentials were not configured. The exact build and strip steps are in the root README.
