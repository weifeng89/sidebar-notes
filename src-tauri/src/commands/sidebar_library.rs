#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn real_directories_and_markdown_are_discoverable() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        initialize(&root).unwrap();
        for category in ["Prompt", "Notes", "Docs"] {
            assert!(root.join(category).is_dir());
        }

        fs::create_dir_all(root.join("Prompt/GTM")).unwrap();
        fs::write(
            root.join("Prompt/GTM/launch.md"),
            "# Launch\nMarket entry plan",
        )
        .unwrap();
        let rows = list(&root, Some("Prompt"), Some("market entry")).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].relative_path, "Prompt/GTM/launch.md");
        assert!(rows[0].is_markdown);
    }

    #[test]
    fn import_preserves_original_format_and_avoids_collisions() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        initialize(&root).unwrap();
        let source = temp.path().join("deck.pdf");
        fs::write(&source, b"%PDF-example").unwrap();

        let first = import_file(&root, &source, "Docs/GTM").unwrap();
        let second = import_file(&root, &source, "Docs/GTM").unwrap();
        assert_ne!(first.relative_path, second.relative_path);
        assert_eq!(
            fs::read(root.join(&first.relative_path)).unwrap(),
            b"%PDF-example"
        );
        assert_eq!(
            fs::read(root.join(&second.relative_path)).unwrap(),
            b"%PDF-example"
        );
    }

    #[test]
    fn artifact_description_search_returns_the_original_file() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        initialize(&root).unwrap();
        let source = temp.path().join("deck.pdf");
        fs::write(&source, b"%PDF-example").unwrap();
        import_path(&root, &source, "Docs/GTM", "launch strategy", None).unwrap();
        let rows = list(&root, Some("Docs"), Some("launch strategy")).unwrap();
        assert!(rows
            .iter()
            .any(|row| row.relative_path == "Docs/GTM/deck.pdf"));
    }

    #[test]
    fn external_edit_never_overwrites_an_unsaved_draft() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        initialize(&root).unwrap();
        let note = create_note(&root, "Notes/Client", "first version").unwrap();
        let opened = read_note(&root, &note.relative_path).unwrap();
        fs::write(root.join(&note.relative_path), "external version").unwrap();

        let saved = save_note(
            &root,
            &note.relative_path,
            "unsaved draft",
            &opened.revision,
        )
        .unwrap();
        let SaveOutcome::Conflict { copy } = saved else {
            panic!("expected a conflict copy")
        };
        assert_eq!(
            fs::read_to_string(root.join(&note.relative_path)).unwrap(),
            "external version"
        );
        assert_eq!(
            fs::read_to_string(root.join(copy)).unwrap(),
            "unsaved draft"
        );
    }

    #[test]
    fn paths_cannot_leave_the_library_or_follow_a_symlink() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        initialize(&root).unwrap();
        assert!(create_note(&root, "../outside", "bad").is_err());
        assert!(read_note(&root, "Prompt/../../outside.md").is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(temp.path(), root.join("Docs/escape")).unwrap();
            assert!(import_file(&root, &temp.path().join("source.pdf"), "Docs/escape").is_err());
        }
    }
}
// The separate, plain-file library used by the persistent sidebar.
// Every public path is relative to the library root and must begin with a category.
use chrono::{DateTime, Local};
use notify_debouncer_mini::new_debouncer;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use std::time::SystemTime;
use tauri::{AppHandle, Emitter};

const CATEGORIES: [&str; 3] = ["Prompt", "Notes", "Docs"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryItem {
    pub relative_path: String,
    pub name: String,
    pub folder: String,
    pub is_markdown: bool,
    pub preview: String,
    pub modified: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteDocument {
    pub content: String,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SaveOutcome {
    Saved { revision: String },
    Conflict { copy: String },
}

pub fn default_root() -> Result<PathBuf, String> {
    if let Some(dev) = super::paths::dev_root()? {
        return Ok(dev.join("sidebar-notes"));
    }
    Ok(dirs::document_dir()
        .ok_or("Cannot find the Documents directory")?
        .join("SideNotes"))
}

pub fn initialize(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    if fs::symlink_metadata(root)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("The library root cannot be a symlink".into());
    }
    for category in CATEGORIES {
        ensure_folder(root, category)?;
    }
    Ok(())
}

fn parts(relative: &str) -> Result<Vec<String>, String> {
    let path = Path::new(relative);
    if path.is_absolute() || relative.is_empty() {
        return Err("Use a relative library path".into());
    }
    let mut result = Vec::new();
    for part in path.components() {
        match part {
            Component::Normal(value) => {
                let text = value.to_str().ok_or("Library paths must be UTF-8")?;
                if text.starts_with('.') || text.is_empty() {
                    return Err("Hidden library paths are not allowed".into());
                }
                result.push(text.to_string());
            }
            _ => return Err("Library paths cannot traverse directories".into()),
        }
    }
    if !result
        .first()
        .is_some_and(|first| CATEGORIES.contains(&first.as_str()))
    {
        return Err("Library path must start with Prompt, Notes, or Docs".into());
    }
    Ok(result)
}

pub fn safe_log_path(relative: &str) -> Option<&str> {
    parts(relative).ok().map(|_| relative)
}

fn existing_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let mut path = root.to_path_buf();
    for part in parts(relative)? {
        path.push(part);
        let kind = fs::symlink_metadata(&path)
            .map_err(|e| e.to_string())?
            .file_type();
        if kind.is_symlink() {
            return Err("Symlinks are not allowed in the library".into());
        }
    }
    Ok(path)
}

fn ensure_folder(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let mut path = root.to_path_buf();
    for part in parts(relative)? {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                return Err("A library folder is not a real directory".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&path).map_err(|e| e.to_string())?;
            }
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(path)
}

fn relative_string(root: &Path, path: &Path) -> Result<String, String> {
    Ok(path
        .strip_prefix(root)
        .map_err(|_| "File is outside the library")?
        .to_string_lossy()
        .replace('\\', "/"))
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
}

fn revision(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn item(root: &Path, path: &Path) -> Result<LibraryItem, String> {
    let relative_path = relative_string(root, path)?;
    let folder = path
        .parent()
        .and_then(|p| p.strip_prefix(root).ok())
        .unwrap_or(Path::new(""))
        .to_string_lossy()
        .replace('\\', "/");
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    let modified: DateTime<Local> =
        DateTime::from(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH));
    let sidecar = path.with_file_name(format!(
        "{}.md",
        path.file_name().unwrap_or_default().to_string_lossy()
    ));
    let description = if is_markdown(path) {
        None
    } else {
        fs::read_to_string(sidecar).ok()
    };
    let preview = if is_markdown(path) {
        fs::read_to_string(path)
            .unwrap_or_default()
            .chars()
            .take(180)
            .collect()
    } else {
        description
            .as_deref()
            .unwrap_or_default()
            .chars()
            .take(180)
            .collect()
    };
    Ok(LibraryItem {
        relative_path,
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        folder,
        is_markdown: is_markdown(path),
        preview,
        modified: modified.to_rfc3339(),
        source: description.and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("来源："))
                .filter(|value| *value != "待确认")
                .map(str::to_string)
        }),
    })
}

pub fn list(
    root: &Path,
    category: Option<&str>,
    query: Option<&str>,
) -> Result<Vec<LibraryItem>, String> {
    initialize(root)?;
    let categories: Vec<&str> = match category {
        Some(value) if CATEGORIES.contains(&value) => vec![value],
        Some(_) => return Err("Unknown category".into()),
        None => CATEGORIES.to_vec(),
    };
    let needle = query.unwrap_or_default().trim().to_lowercase();
    let mut rows = Vec::new();
    for category in categories {
        let mut pending = vec![root.join(category)];
        while let Some(folder) = pending.pop() {
            for entry in fs::read_dir(&folder).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                if entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                let kind = entry.file_type().map_err(|e| e.to_string())?;
                if kind.is_symlink() {
                    continue;
                }
                if kind.is_dir() {
                    pending.push(entry.path());
                    continue;
                }
                if !kind.is_file() {
                    continue;
                }
                let row = item(root, &entry.path())?;
                if needle.is_empty()
                    || row.name.to_lowercase().contains(&needle)
                    || row.preview.to_lowercase().contains(&needle)
                    || (row.is_markdown
                        && fs::read_to_string(entry.path())
                            .unwrap_or_default()
                            .to_lowercase()
                            .contains(&needle))
                    || (!row.is_markdown
                        && fs::read_to_string(
                            entry.path().with_file_name(format!("{}.md", row.name)),
                        )
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&needle))
                {
                    rows.push(row);
                }
            }
        }
    }
    rows.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });
    Ok(rows)
}

pub fn list_folders(root: &Path) -> Result<Vec<String>, String> {
    initialize(root)?;
    let mut result = Vec::new();
    let mut pending: Vec<PathBuf> = CATEGORIES.iter().map(|name| root.join(name)).collect();
    while let Some(folder) = pending.pop() {
        result.push(relative_string(root, &folder)?);
        for entry in fs::read_dir(&folder).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_name().to_string_lossy().starts_with('.')
                && entry.file_type().map_err(|e| e.to_string())?.is_dir()
            {
                pending.push(entry.path());
            }
        }
    }
    result.sort();
    Ok(result)
}

fn unique_path(folder: &Path, name: &str) -> PathBuf {
    let source = Path::new(name);
    let stem = source.file_stem().unwrap_or_default().to_string_lossy();
    let extension = source.extension().and_then(|e| e.to_str());
    for index in 0.. {
        let candidate = if index == 0 {
            name.to_string()
        } else {
            match extension {
                Some(ext) => format!("{stem}-{index}.{ext}"),
                None => format!("{stem}-{index}"),
            }
        };
        let path = folder.join(candidate);
        if !path.exists() {
            return path;
        }
    }
    unreachable!()
}

pub fn create_note(root: &Path, folder: &str, content: &str) -> Result<LibraryItem, String> {
    initialize(root)?;
    let directory = ensure_folder(root, folder)?;
    let filename = format!("{}.md", Local::now().format("%Y-%m-%d-%H%M"));
    let path = unique_path(&directory, &filename);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    file.write_all(content.as_bytes())
        .map_err(|e| e.to_string())?;
    item(root, &path)
}

pub fn import_file(root: &Path, source: &Path, folder: &str) -> Result<LibraryItem, String> {
    initialize(root)?;
    if !folder.starts_with("Docs") || !(folder == "Docs" || folder.starts_with("Docs/")) {
        return Err("Artifacts belong in Docs".into());
    }
    if !fs::symlink_metadata(source)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_file()
    {
        return Err("Import source must be a regular file".into());
    }
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid source filename")?;
    if name.starts_with('.') {
        return Err("Hidden files cannot be imported".into());
    }
    let directory = ensure_folder(root, folder)?;
    let destination = unique_path(&directory, name);
    let mut input = File::open(source).map_err(|e| e.to_string())?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(|e| e.to_string())?;
    std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
    item(root, &destination)
}

pub fn read_note(root: &Path, relative: &str) -> Result<NoteDocument, String> {
    let path = existing_path(root, relative)?;
    if !is_markdown(&path) {
        return Err("Only Markdown files can be edited".into());
    }
    let data = fs::read(path).map_err(|e| e.to_string())?;
    Ok(NoteDocument {
        content: String::from_utf8(data.clone()).map_err(|e| e.to_string())?,
        revision: revision(&data),
    })
}

pub fn save_note(
    root: &Path,
    relative: &str,
    content: &str,
    expected_revision: &str,
) -> Result<SaveOutcome, String> {
    let path = existing_path(root, relative)?;
    if !is_markdown(&path) {
        return Err("Only Markdown files can be edited".into());
    }
    let previous = fs::read(&path).map_err(|e| e.to_string())?;
    if revision(&previous) != expected_revision {
        let parent = path.parent().ok_or("Note has no parent folder")?;
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let name = format!(
            "{stem}-conflict-{}.md",
            Local::now().format("%Y%m%d-%H%M%S")
        );
        let copy = unique_path(parent, &name);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&copy)
            .map_err(|e| e.to_string())?;
        file.write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
        return Ok(SaveOutcome::Conflict {
            copy: relative_string(root, &copy)?,
        });
    }
    let parent = path.parent().ok_or("Note has no parent folder")?;
    let temp = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        uuid::Uuid::new_v4()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    if let Err(error) = file
        .write_all(content.as_bytes())
        .and_then(|_| file.sync_all())
        .and_then(|_| fs::rename(&temp, &path))
    {
        let _ = fs::remove_file(&temp);
        return Err(error.to_string());
    }
    Ok(SaveOutcome::Saved {
        revision: revision(content.as_bytes()),
    })
}

fn validate_source_url(value: &str) -> Result<(), String> {
    if value.len() > 2048
        || value.chars().any(char::is_control)
        || !(value.starts_with("https://") || value.starts_with("codex://threads/"))
    {
        return Err("Source must be an HTTPS chat link or a Codex thread link".into());
    }
    Ok(())
}

fn write_description(
    root: &Path,
    artifact: &LibraryItem,
    description: &str,
    source: Option<&str>,
) -> Result<(), String> {
    let artifact_path = root.join(&artifact.relative_path);
    let sidecar = artifact_path.with_file_name(format!("{}.md", artifact.name));
    let source_text = source.unwrap_or("待确认");
    let body = format!(
        "# {}\n\n{}\n\n来源：{}\n",
        artifact.name,
        description.trim(),
        source_text
    );
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(sidecar)
        .map_err(|e| e.to_string())?;
    output.write_all(body.as_bytes()).map_err(|e| e.to_string())
}

pub fn import_path(
    root: &Path,
    source: &Path,
    folder: &str,
    description: &str,
    source_url: Option<&str>,
) -> Result<Vec<LibraryItem>, String> {
    if let Some(url) = source_url {
        validate_source_url(url)?;
    }
    let metadata = fs::symlink_metadata(source).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err("Symlink imports are not allowed".into());
    }
    if metadata.is_file() {
        let imported = import_file(root, source, folder)?;
        write_description(root, &imported, description, source_url)?;
        return Ok(vec![imported]);
    }
    if !metadata.is_dir() {
        return Err("Import source must be a file or directory".into());
    }
    let folder_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Invalid folder name")?;
    if folder_name.starts_with('.') {
        return Err("Hidden folders cannot be imported".into());
    }
    let parent = ensure_folder(root, folder)?;
    let destination = unique_path(&parent, folder_name);
    let target_folder = relative_string(root, &destination)?;
    ensure_folder(root, &target_folder)?;
    let mut imported = Vec::new();
    let mut pending = vec![(source.to_path_buf(), target_folder)];
    while let Some((directory, destination_folder)) = pending.pop() {
        for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                let child_folder = format!("{destination_folder}/{name}");
                ensure_folder(root, &child_folder)?;
                pending.push((entry.path(), child_folder));
            } else if kind.is_file() {
                let item = import_file(root, &entry.path(), &destination_folder)?;
                write_description(root, &item, description, source_url)?;
                imported.push(item);
            }
        }
    }
    Ok(imported)
}

#[tauri::command]
pub fn sidebar_list(category: String, query: String) -> Result<Vec<LibraryItem>, String> {
    crate::sidebar_log::trace(
        "list",
        parts(&category).ok().map(|_| category.as_str()),
        || list(&default_root()?, Some(&category), Some(&query)),
    )
}

#[tauri::command]
pub fn sidebar_list_folders() -> Result<Vec<String>, String> {
    crate::sidebar_log::trace("list_folders", None, || list_folders(&default_root()?))
}

#[tauri::command]
pub fn sidebar_read_note(relative_path: String) -> Result<NoteDocument, String> {
    crate::sidebar_log::trace(
        "read_note",
        parts(&relative_path).ok().map(|_| relative_path.as_str()),
        || {
            let root = default_root()?;
            initialize(&root)?;
            read_note(&root, &relative_path)
        },
    )
}

#[tauri::command]
pub fn sidebar_save_note(
    relative_path: String,
    content: String,
    expected_revision: String,
) -> Result<SaveOutcome, String> {
    crate::sidebar_log::trace(
        "save_note",
        parts(&relative_path).ok().map(|_| relative_path.as_str()),
        || {
            let root = default_root()?;
            initialize(&root)?;
            save_note(&root, &relative_path, &content, &expected_revision)
        },
    )
}

#[tauri::command]
pub fn sidebar_create_note(folder: String) -> Result<LibraryItem, String> {
    crate::sidebar_log::trace(
        "create_note",
        parts(&folder).ok().map(|_| folder.as_str()),
        || create_note(&default_root()?, &folder, ""),
    )
}

#[tauri::command]
pub fn sidebar_create_folder(folder: String) -> Result<String, String> {
    crate::sidebar_log::trace(
        "create_folder",
        parts(&folder).ok().map(|_| folder.as_str()),
        || {
            let root = default_root()?;
            initialize(&root)?;
            let path = ensure_folder(&root, &folder)?;
            relative_string(&root, &path)
        },
    )
}

#[tauri::command]
pub fn sidebar_import_file(
    source_path: String,
    folder: String,
    description: String,
    source_url: Option<String>,
) -> Result<Vec<LibraryItem>, String> {
    crate::sidebar_log::trace(
        "import",
        parts(&folder).ok().map(|_| folder.as_str()),
        || {
            let root = default_root()?;
            initialize(&root)?;
            import_path(
                &root,
                Path::new(&source_path),
                &folder,
                &description,
                source_url.as_deref(),
            )
        },
    )
}

#[tauri::command]
pub fn sidebar_open_file(relative_path: String) -> Result<(), String> {
    let root = default_root()?;
    let path = existing_path(&root, &relative_path)?;
    std::process::Command::new("open")
        .arg(path)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err("Could not open file".into())
            }
        })
}

#[tauri::command]
pub fn sidebar_show_in_finder(relative_path: String) -> Result<(), String> {
    let root = default_root()?;
    let path = existing_path(&root, &relative_path)?;
    std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err("Could not reveal file".into())
            }
        })
}

#[tauri::command]
pub fn sidebar_open_source(relative_path: String) -> Result<(), String> {
    let root = default_root()?;
    let path = existing_path(&root, &relative_path)?;
    let source = item(&root, &path)?.source.ok_or("No confirmed source")?;
    validate_source_url(&source)?;
    std::process::Command::new("open")
        .arg(source)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err("Could not open source".into())
            }
        })
}

static WATCHER_STARTED: OnceLock<()> = OnceLock::new();

pub fn start_watcher(app: AppHandle) {
    if WATCHER_STARTED.set(()).is_err() {
        return;
    }
    let Ok(root) = default_root() else {
        return;
    };
    std::thread::Builder::new()
        .name("sidebar-file-watcher".into())
        .spawn(move || loop {
            if initialize(&root).is_err() {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            let (tx, rx) = std::sync::mpsc::channel();
            let mut debouncer = match new_debouncer(Duration::from_millis(180), tx) {
                Ok(value) => value,
                Err(_) => {
                    let _ = crate::sidebar_log::trace::<()>("watch", None, || {
                        Err("watcher_create_failed".into())
                    });
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
            };
            if debouncer
                .watcher()
                .watch(&root, notify::RecursiveMode::Recursive)
                .is_err()
            {
                let _ = crate::sidebar_log::trace::<()>("watch", None, || {
                    Err("watcher_attach_failed".into())
                });
                std::thread::sleep(Duration::from_secs(1));
                continue;
            }
            while let Ok(event) = rx.recv() {
                match event {
                    Ok(changes) if !changes.is_empty() => {
                        let _ = app.emit("sidebar-files-changed", ());
                    }
                    Ok(_) => {}
                    Err(_) => {
                        let _ = crate::sidebar_log::trace::<()>("watch", None, || {
                            Err("watcher_event_failed".into())
                        });
                        break;
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        })
        .ok();
}
