# Local agent interface

Run the app executable with `cli`. For a development build, replace `APP` with the absolute path to `stik` inside the built `.app/Contents/MacOS/` directory. The production bundle will document its executable path when packaged.

```sh
"$APP" cli --json folders
"$APP" cli --json list Prompt
"$APP" cli --json search 'GTM'
"$APP" cli --json show 'Prompt/GTM/template.md'
"$APP" cli --json create 'Notes/Client'
printf '%s' '# Updated note' | "$APP" cli --json save 'Notes/Client/note.md' REVISION_FROM_SHOW
"$APP" cli --json import '/path/to/deck.pdf' 'Docs/Client' 'GTM deck' 'https://example.com/chat'
```

`--json` prints a single JSON value to standard output. Errors print a JSON object with `error_code` and `message` to standard error and exit 2. A save conflict exits 3 and returns the `conflict` result with a second library-relative Markdown path; both versions stay on disk. The CLI works without opening the UI. It uses the same default library root as the app. Development builds honor `STIK_DEV_ROOT` and keep test files in `STIK_DEV_ROOT/sidebar-notes`.

For agents, read the item and capture its `revision` before saving. Supply the complete new Markdown content on standard input. Use the returned conflict path if the file changed after it was read. An imported artifact remains in its original format under `Docs`; the app adds a neighboring Markdown description. Source attribution is unconfirmed unless the user supplies or approves the link.
