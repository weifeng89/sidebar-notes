# Agent interface: Mac-side CLI and remote access

The canonical library lives only on the Mac. Agents running on that Mac can use `stik cli` below. Agents on another machine use `scripts/sidebarnotes-remote.py`, which calls the Mac app's `remote` subcommand over SSH. The client keeps no library or cache on the remote machine. See [ADR 0003](adr/0003-agent-interface.md) for the trust boundary.

## Remote agents

Copy `scripts/sidebarnotes-remote.py` to the agent machine and give it an SSH alias for the Mac. Install the current app bundle on the Mac at `/Applications/Sidebar Notes.app`, or set `SIDEBAR_NOTES_MAC_APP` to its actual executable path. Use a dedicated SSH key whose Mac `authorized_keys` entry uses OpenSSH's `restrict` and forced `command` options, with the forced command set to the app executable followed by `remote`. This key must not grant an ordinary shell. The Mac must allow SSH from the trusted private network; do not expose port 22 publicly. Keep private keys out of the repository and agent prompts.

```sh
export SIDEBAR_NOTES_SSH_TARGET=sidebarnotes-mac
python3 sidebarnotes-remote.py folders
python3 sidebarnotes-remote.py search GTM
python3 sidebarnotes-remote.py show 'Prompt/GTM/template.md'
python3 sidebarnotes-remote.py create 'Notes/Client'
printf '%s' '# Updated note' | python3 sidebarnotes-remote.py save 'Notes/Client/note.md' REVISION_FROM_SHOW
python3 sidebarnotes-remote.py import './deck.pdf' 'Docs/Client' 'GTM deck' 'https://example.com/chat'
```

The SSH target is a host alias, not a library path. The client always returns JSON on standard output and propagates the Mac CLI's exit status: 0 for success, 2 for error, 3 for a preserved save conflict. It sends Markdown or original artifact bytes through the SSH channel; artifact paths on the agent machine are never interpreted as paths on the Mac. Maximum transfer sizes are 8 MiB per note and 512 MiB per artifact. An interrupted transfer leaves no partial file in the library. The remote protocol permits only folder/list/search/show/create/save/import operations; it does not accept arbitrary filesystem paths for import.

To accept this feature, run from the actual remote host against a disposable `STIK_DEV_ROOT` debug profile on the Mac and verify search, create, show, save, conflict copy, and artifact import. A local fake-SSH test exercises framing but does not prove reachability or key restrictions. Release builds reject `STIK_DEV_ROOT`; never run unattended tests against personal notes.

Run the app executable with `cli`. For a development build, replace `APP` with the absolute path to `stik` inside the built `.app/Contents/MacOS/` directory. The production bundle will document its executable path when packaged.

```sh
"$APP" cli --json folders
"$APP" cli --json mkdir 'Prompt/Client'
"$APP" cli --json list Prompt
"$APP" cli --json search 'GTM'
"$APP" cli --json show 'Prompt/GTM/template.md'
"$APP" cli --json create 'Notes/Client'
printf '%s' '# Updated note' | "$APP" cli --json save 'Notes/Client/note.md' REVISION_FROM_SHOW
"$APP" cli --json import '/path/to/deck.pdf' 'Docs/Client' 'GTM deck' 'https://example.com/chat'
```

`--json` prints a single JSON value to standard output. Errors print a JSON object with `error_code` and `message` to standard error and exit 2. A save conflict exits 3 and returns the `conflict` result with a second library-relative Markdown path; both versions stay on disk. The CLI works without opening the UI. It uses the same default library root as the app. Development builds honor `STIK_DEV_ROOT` and keep test files in `STIK_DEV_ROOT/sidebar-notes`.

For agents, read a note and capture its `revision` before saving. Supply the complete new Markdown content on standard input. Use the returned conflict path if the file changed after it was read. An imported artifact remains in its original format under `Docs`; the app adds a neighboring Markdown description and returns the artifact with its description and source metadata. Source attribution is unconfirmed unless the user supplies or approves the link. Import files one at a time when each needs a distinct description or source. `mkdir` creates an empty real directory without creating a note.

`show` includes the body and revision for any Markdown file, including a Markdown artifact in `Docs`. For other artifact formats it returns metadata and description; agents can open the on-disk original with a format-specific reader.
