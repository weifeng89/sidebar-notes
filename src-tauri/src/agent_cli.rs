use crate::commands::sidebar_library as library;
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;

const USAGE: &str = "Usage: stik cli [--json] <list [Prompt|Notes|Docs] | search QUERY | show RELATIVE_PATH | folders | create FOLDER | save RELATIVE_PATH EXPECTED_REVISION < CONTENT | import SOURCE_PATH DOCS_FOLDER [DESCRIPTION] [SOURCE_URL]>";

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
        ["show", relative] => {
            let item = library::list(root, None, None)?
                .into_iter()
                .find(|item| item.relative_path == *relative)
                .ok_or("Library item not found")?;
            if item.is_markdown {
                let note = library::read_note(root, relative)?;
                Ok(
                    json!({"relative_path": relative, "content": note.content, "revision": note.revision}),
                )
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
        ["show" | "save", path, ..] | ["create", path] | ["import", _, path, ..] => Some(*path),
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
        assert_eq!(
            std::fs::read(root.join("Docs/GTM/deck.pdf")).unwrap(),
            b"%PDF-test"
        );
        assert!(execute(&root, &["show", "../../outside.md"], None).is_err());
        assert!(execute(&root, &["create", "Notes/../../outside"], None).is_err());
    }
}
