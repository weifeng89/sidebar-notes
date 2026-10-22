use crate::commands::sidebar_library as library;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::Path;

const MAX_HEADER_BYTES: usize = 8192;
const MAX_NOTE_BYTES: u64 = 8 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoteRequest {
    version: u8,
    command: Vec<String>,
    payload_bytes: u64,
}

fn remote_header(reader: &mut impl Read) -> Result<RemoteRequest, String> {
    let mut header = Vec::new();
    for _ in 0..MAX_HEADER_BYTES {
        let mut byte = [0];
        reader
            .read_exact(&mut byte)
            .map_err(|_| "Incomplete remote header")?;
        if byte[0] == b'\n' {
            let request: RemoteRequest =
                serde_json::from_slice(&header).map_err(|_| "Invalid remote header")?;
            if request.version != 1 {
                return Err("Unsupported remote protocol version".into());
            }
            return Ok(request);
        }
        header.push(byte[0]);
    }
    Err("Remote header is too long".into())
}

fn remote_filename(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 255
        || name.starts_with('.')
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.chars().any(char::is_control)
    {
        return Err("Import requires a regular filename".into());
    }
    Ok(())
}

fn execute_remote(root: &Path, reader: &mut impl Read) -> Result<Value, String> {
    let request = remote_header(reader)?;
    let args: Vec<&str> = request.command.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["save", relative, expected_revision] => {
            if request.payload_bytes > MAX_NOTE_BYTES {
                return Err("Remote note exceeds 8 MiB".into());
            }
            let mut content = vec![0; request.payload_bytes as usize];
            reader
                .read_exact(&mut content)
                .map_err(|_| "Incomplete remote payload")?;
            let content = String::from_utf8(content).map_err(|_| "Remote note must be UTF-8")?;
            execute(root, &["save", relative, expected_revision], Some(&content))
        }
        ["import", name, folder, rest @ ..] if rest.len() <= 2 => {
            remote_filename(name)?;
            if request.payload_bytes > MAX_ARTIFACT_BYTES {
                return Err("Remote artifact exceeds 512 MiB".into());
            }
            let staging = tempfile::tempdir().map_err(|e| e.to_string())?;
            let source = staging.path().join(name);
            let mut file = std::fs::File::create(&source).map_err(|e| e.to_string())?;
            let copied = std::io::copy(&mut reader.take(request.payload_bytes), &mut file)
                .map_err(|e| e.to_string())?;
            if copied != request.payload_bytes {
                return Err("Incomplete remote payload".into());
            }
            file.flush().map_err(|e| e.to_string())?;
            drop(file);
            serde_json::to_value(library::import_path(
                root,
                &source,
                folder,
                rest.first().copied().unwrap_or(""),
                rest.get(1).copied(),
            )?)
            .map_err(|e| e.to_string())
        }
        ["list"]
        | ["list", _]
        | ["search", _]
        | ["show", _]
        | ["folders"]
        | ["mkdir", _]
        | ["create", _]
            if request.payload_bytes == 0 =>
        {
            execute(root, &args, None)
        }
        _ => Err("Remote command or payload is not allowed".into()),
    }
}

pub fn run_remote() -> i32 {
    let outcome = crate::sidebar_log::trace("agent_remote", None, || {
        execute_remote(&library::default_root()?, &mut std::io::stdin().lock())
    });
    match outcome {
        Ok(value) => {
            println!("{value}");
            if value["status"] == "conflict" {
                3
            } else {
                0
            }
        }
        Err(message) => {
            eprintln!(
                "{}",
                json!({"error_code":"remote_operation_failed","message":message})
            );
            2
        }
    }
}

const USAGE: &str = "Usage: stik cli [--json] <list [Prompt|Notes|Docs] | search QUERY | show RELATIVE_PATH | folders | mkdir FOLDER | create FOLDER | save RELATIVE_PATH EXPECTED_REVISION < CONTENT | import SOURCE_PATH DOCS_FOLDER [DESCRIPTION] [SOURCE_URL]>";

fn execute(root: &Path, args: &[&str], stdin: Option<&str>) -> Result<Value, String> {
    library::initialize(root)?;
    match args {
        ["list"] => {
            serde_json::to_value(library::list(root, None, None)?).map_err(|e| e.to_string())
        }
        ["list", category] => serde_json::to_value(library::list(root, Some(category), None)?)
            .map_err(|e| e.to_string()),
        ["search", query] => {
            serde_json::to_value(library::list(root, None, Some(query))?).map_err(|e| e.to_string())
        }
        ["folders"] => {
            serde_json::to_value(library::list_folders(root)?).map_err(|e| e.to_string())
        }
        ["mkdir", folder] => Ok(json!({"folder": library::create_folder(root, folder)?})),
        ["show", relative] => {
            let item = library::list(root, None, None)?
                .into_iter()
                .find(|item| item.relative_path == *relative)
                .ok_or("Library item not found")?;
            if item.is_markdown {
                let note = library::read_note(root, relative)?;
                let mut value = serde_json::to_value(item).map_err(|e| e.to_string())?;
                value["content"] = json!(note.content);
                value["revision"] = json!(note.revision);
                Ok(value)
            } else {
                serde_json::to_value(item).map_err(|e| e.to_string())
            }
        }
        ["create", folder] => {
            serde_json::to_value(library::create_note(root, folder, "")?).map_err(|e| e.to_string())
        }
        ["save", relative, expected_revision] => {
            let content = stdin.ok_or("Save requires content on standard input")?;
            serde_json::to_value(library::save_note(
                root,
                relative,
                content,
                expected_revision,
            )?)
            .map_err(|e| e.to_string())
        }
        ["import", source, folder] => serde_json::to_value(library::import_path(
            root,
            Path::new(source),
            folder,
            "",
            None,
        )?)
        .map_err(|e| e.to_string()),
        ["import", source, folder, description] => serde_json::to_value(library::import_path(
            root,
            Path::new(source),
            folder,
            description,
            None,
        )?)
        .map_err(|e| e.to_string()),
        ["import", source, folder, description, source_url] => {
            serde_json::to_value(library::import_path(
                root,
                Path::new(source),
                folder,
                description,
                Some(source_url),
            )?)
            .map_err(|e| e.to_string())
        }
        _ => Err(USAGE.into()),
    }
}

pub fn run_cli(args: &[String]) -> i32 {
    let json_output = args.iter().any(|arg| arg == "--json");
    let values: Vec<&str> = args
        .iter()
        .filter(|arg| *arg != "--json")
        .map(String::as_str)
        .collect();
    if values.is_empty() || values[0] == "help" || values[0] == "--help" {
        println!("{USAGE}");
        return if values.is_empty() { 2 } else { 0 };
    }
    let mut input = String::new();
    if values[0] == "save" {
        if let Err(error) = std::io::stdin().read_to_string(&mut input) {
            eprintln!(
                "{}",
                json!({"error_code":"input_failed","message":error.to_string()})
            );
            return 2;
        }
    }
    let relative = match values.as_slice() {
        ["show" | "save", path, ..] | ["create" | "mkdir", path] | ["import", _, path, ..] => {
            Some(*path)
        }
        _ => None,
    };
    let outcome = crate::sidebar_log::trace(
        "agent_cli",
        relative.and_then(library::safe_log_path),
        || {
            execute(
                &library::default_root()?,
                &values,
                if values[0] == "save" {
                    Some(&input)
                } else {
                    None
                },
            )
        },
    );
    match outcome {
        Ok(value) => {
            if json_output {
                println!("{value}");
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&value).unwrap_or_default()
                );
            }
            if value["status"] == "conflict" {
                3
            } else {
                0
            }
        }
        Err(message) => {
            let code = if message == USAGE {
                "invalid_arguments"
            } else {
                "operation_failed"
            };
            if json_output {
                eprintln!("{}", json!({"error_code":code,"message":message}));
            } else {
                eprintln!("{code}: {message}");
            }
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn remote_agent_uses_the_mac_library_for_reads_writes_and_conflicts() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        let create = br#"{"version":1,"command":["create","Notes/Remote"],"payload_bytes":0}
"#;
        let created = execute_remote(&root, &mut create.as_slice()).unwrap();
        let path = created["relative_path"].as_str().unwrap();
        let show =
            format!("{{\"version\":1,\"command\":[\"show\",\"{path}\"],\"payload_bytes\":0}}\n");
        let opened = execute_remote(&root, &mut show.as_bytes()).unwrap();
        let revision = opened["revision"].as_str().unwrap();
        let body = b"# Remote note";
        let save = format!("{{\"version\":1,\"command\":[\"save\",\"{path}\",\"{revision}\"],\"payload_bytes\":{}}}\n", body.len());
        let request = [save.as_bytes(), body].concat();
        assert_eq!(
            execute_remote(&root, &mut request.as_slice()).unwrap()["status"],
            "saved"
        );
        let stale_body = b"agent draft";
        let stale_header = format!("{{\"version\":1,\"command\":[\"save\",\"{path}\",\"{revision}\"],\"payload_bytes\":{}}}\n", stale_body.len());
        let stale = [stale_header.as_bytes(), stale_body].concat();
        let conflict = execute_remote(&root, &mut stale.as_slice()).unwrap();
        assert_eq!(conflict["status"], "conflict");
        assert_eq!(
            std::fs::read_to_string(root.join(conflict["copy"].as_str().unwrap())).unwrap(),
            "agent draft"
        );
        assert_eq!(
            std::fs::read_to_string(root.join(path)).unwrap(),
            "# Remote note"
        );
        let truncated = [stale_header.as_bytes(), b"short"].concat();
        assert!(execute_remote(&root, &mut truncated.as_slice()).is_err());
    }

    #[test]
    fn remote_agent_streams_artifact_bytes_without_a_remote_path_on_the_mac() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        let body = b"%PDF-remote";
        let header = format!("{{\"version\":1,\"command\":[\"import\",\"deck.pdf\",\"Docs/Client\",\"Handoff\"],\"payload_bytes\":{}}}\n", body.len());
        let request = [header.as_bytes(), body].concat();
        let result = execute_remote(&root, &mut request.as_slice()).unwrap();
        assert_eq!(result[0]["relative_path"], "Docs/Client/deck.pdf");
        assert_eq!(
            std::fs::read(root.join("Docs/Client/deck.pdf")).unwrap(),
            body
        );
        let invalid = br#"{"version":1,"command":["import","/etc/passwd","Docs"],"payload_bytes":0}
"#;
        assert!(execute_remote(&root, &mut invalid.as_slice()).is_err());
    }

    #[test]
    fn agent_can_create_find_and_read_a_markdown_note() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        let created = execute(&root, &["create", "Prompt/GTM"], None).unwrap();
        let path = created["relative_path"].as_str().unwrap();
        let opened = execute(&root, &["show", path], None).unwrap();
        let revision = opened["revision"].as_str().unwrap();
        let saved = execute(
            &root,
            &["save", path, revision],
            Some("# GTM\nMarket entry"),
        )
        .unwrap();
        assert_eq!(saved["status"], "saved");
        let found = execute(&root, &["search", "market entry"], None).unwrap();
        assert_eq!(found.as_array().unwrap().len(), 1);
        assert_eq!(found[0]["relative_path"], path);
    }

    #[test]
    fn agent_save_preserves_an_external_edit_as_a_conflict() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        let created = execute(&root, &["create", "Notes"], None).unwrap();
        let path = created["relative_path"].as_str().unwrap();
        let revision = execute(&root, &["show", path], None).unwrap()["revision"]
            .as_str()
            .unwrap()
            .to_string();
        std::fs::write(root.join(path), "external").unwrap();
        let result = execute(&root, &["save", path, &revision], Some("agent draft")).unwrap();
        assert_eq!(result["status"], "conflict");
        assert_eq!(
            std::fs::read_to_string(root.join(path)).unwrap(),
            "external"
        );
        let copy = result["copy"].as_str().unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join(copy)).unwrap(),
            "agent draft"
        );
    }

    #[test]
    fn agent_import_preserves_artifact_and_rejects_paths_outside_library() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        let source = temp.path().join("deck.pdf");
        std::fs::write(&source, b"%PDF-test").unwrap();
        let imported = execute(
            &root,
            &[
                "import",
                source.to_str().unwrap(),
                "Docs/GTM",
                "Launch deck",
            ],
            None,
        )
        .unwrap();
        assert_eq!(imported[0]["relative_path"], "Docs/GTM/deck.pdf");
        assert_eq!(imported[0]["description"], "Launch deck");
        assert_eq!(
            std::fs::read(root.join("Docs/GTM/deck.pdf")).unwrap(),
            b"%PDF-test"
        );
        assert!(execute(&root, &["show", "../../outside.md"], None).is_err());
        assert!(execute(&root, &["create", "Notes/../../outside"], None).is_err());
    }

    #[test]
    fn showing_an_imported_markdown_returns_content_and_artifact_metadata() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        let source = temp.path().join("handoff.md");
        std::fs::write(&source, "# Handoff").unwrap();
        execute(
            &root,
            &[
                "import",
                source.to_str().unwrap(),
                "Docs/Client",
                "Final version",
            ],
            None,
        )
        .unwrap();
        let shown = execute(&root, &["show", "Docs/Client/handoff.md"], None).unwrap();
        assert_eq!(shown["description"], "Final version");
        assert_eq!(shown["is_artifact"], true);
        assert_eq!(shown["content"], "# Handoff");
    }

    #[test]
    fn agent_can_create_an_empty_project_directory() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        let result = execute(&root, &["mkdir", "Prompt/Client A"], None).unwrap();
        assert_eq!(result["folder"], "Prompt/Client A");
        assert!(root.join("Prompt/Client A").is_dir());
    }
}
