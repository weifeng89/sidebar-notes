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
    fn search_index_refreshes_after_internal_and_external_edits() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        initialize(&root).unwrap();
        assert!(list(&root, Some("Prompt"), None).unwrap().is_empty());
        let created = create_note(&root, "Prompt/GTM", "initial term").unwrap();
        assert_eq!(
            list(&root, Some("Prompt"), Some("initial term"))
                .unwrap()
                .len(),
            1
        );
        let opened = read_note(&root, &created.relative_path).unwrap();
        save_note(
            &root,
            &created.relative_path,
            "revised term",
            &opened.revision,
        )
        .unwrap();
        assert_eq!(
            list(&root, Some("Prompt"), Some("revised term"))
                .unwrap()
                .len(),
            1
        );
        fs::write(root.join(&created.relative_path), "external term").unwrap();
        invalidate_index(&root);
        assert_eq!(
            list(&root, Some("Prompt"), Some("external term"))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    #[ignore = "manual 10000-file performance gate"]
    fn ten_thousand_file_search_timing() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        let folder = root.join("Prompt/Load");
        fs::create_dir_all(&folder).unwrap();
        for number in 0..10_000 {
            fs::write(
                folder.join(format!("note-{number:05}.md")),
                format!("# Note {number}\nunique term {number}"),
            )
            .unwrap();
        }
        let started = std::time::Instant::now();
        let rows = list(&root, Some("Prompt"), None).unwrap();
        let cold_scan = started.elapsed();
        assert_eq!(rows.len(), 10_000);
        let started = std::time::Instant::now();
        let hits = list(&root, Some("Prompt"), Some("unique term 9999")).unwrap();
        let warm_search = started.elapsed();
        assert_eq!(hits.len(), 1);
        println!(
            "cold_scan_ms={} warm_search_ms={}",
            cold_scan.as_millis(),
            warm_search.as_millis()
        );
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
        assert_eq!(
            rows.len(),
            1,
            "sidecar belongs to its artifact, not a separate row"
        );
        assert_eq!(rows[0].description.as_deref(), Some("launch strategy"));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_description_is_not_read_or_indexed() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        initialize(&root).unwrap();
        let outside = temp.path().join("outside.md");
        fs::write(&outside, "来源：待确认\nprivate-outside-marker").unwrap();
        fs::write(root.join("Docs/deck.pdf"), b"pdf").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("Docs/deck.pdf.md")).unwrap();
        let rows = list(&root, Some("Docs"), None).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].is_artifact);
        assert!(list(&root, None, Some("private-outside-marker"))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn imported_markdown_is_one_artifact_with_its_description() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        initialize(&root).unwrap();
        let source = temp.path().join("deliverable.md");
        fs::write(&source, "# Report\nBody").unwrap();
        import_path(&root, &source, "Docs/Client", "final handoff", None).unwrap();
        let rows = list(&root, Some("Docs"), Some("final handoff")).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "deliverable.md");
        assert!(rows[0].is_artifact);
        assert!(rows[0].is_markdown);
    }

    #[test]
    fn moving_an_artifact_to_trash_keeps_its_description_and_blocks_escape() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        let trash = temp.path().join("Trash");
        fs::create_dir(&trash).unwrap();
        initialize(&root).unwrap();
        let source = temp.path().join("deck.pdf");
        fs::write(&source, b"pdf").unwrap();
        import_path(&root, &source, "Docs/Client", "hand-off", None).unwrap();
        let move_one = |path: &Path| {
            let destination = trash.join(path.file_name().unwrap());
            fs::rename(path, &destination).map_err(|e| e.to_string())?;
            Ok(destination)
        };
        trash_item_with(&root, "Docs/Client/deck.pdf", move_one).unwrap();
        assert!(trash.join("deck.pdf").is_file());
        assert!(trash.join("deck.pdf.md").is_file());
        assert!(list(&root, Some("Docs"), None).unwrap().is_empty());
        assert!(trash_item_with(&root, "../outside", |_| unreachable!()).is_err());
    }

    #[test]
    fn import_candidates_expand_directories_without_symlinks() {
        let temp = tempdir().unwrap();
        let bundle = temp.path().join("bundle");
        fs::create_dir_all(bundle.join("nested")).unwrap();
        fs::write(bundle.join("nested/deck.pdf"), b"pdf").unwrap();
        fs::write(bundle.join(".hidden"), b"hidden").unwrap();
        let candidates = import_candidates(&[bundle]).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].relative_folder, "bundle/nested");
        assert!(candidates[0].source_path.ends_with("nested/deck.pdf"));
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

    #[cfg(target_os = "macos")]
    #[test]
    fn interrupted_save_exposes_hidden_external_version() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("SideNotes");
        initialize(&root).unwrap();
        let original = root.join("Notes/idea.md");
        fs::write(&original, "draft").unwrap();
        let hidden = root.join("Notes/.idea.md.displaced.tmp");
        fs::write(&hidden, "external version").unwrap();
        expose_hidden_conflict(&root, &original, &hidden).unwrap();
        assert!(!hidden.exists());
        let rows = list(&root, Some("Notes"), Some("external version")).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].name.contains("external-conflict"));
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
use std::fs::{self, File};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};
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
    pub is_artifact: bool,
    pub preview: String,
    pub modified: String,
    pub source: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportCandidate {
    pub source_path: String,
    pub relative_folder: String,
}

struct IndexedItem {
    item: LibraryItem,
    searchable: String,
}

struct SearchIndex {
    root: PathBuf,
    dirty: bool,
    items: Vec<IndexedItem>,
}

fn index() -> &'static Mutex<Option<SearchIndex>> {
    static INDEX: OnceLock<Mutex<Option<SearchIndex>>> = OnceLock::new();
    INDEX.get_or_init(|| Mutex::new(None))
}

fn invalidate_index(root: &Path) {
    let mut guard = index().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(current) = guard.as_mut() {
        if current.root == root {
            current.dirty = true;
        }
    }
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

fn read_regular_text(path: &Path) -> Option<String> {
    if !fs::symlink_metadata(path).ok()?.file_type().is_file() {
        return None;
    }
    fs::read_to_string(path).ok()
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
    let description = if relative_path.starts_with("Docs/") {
        read_regular_text(&sidecar)
    } else {
        None
    };
    let is_artifact = description
        .as_ref()
        .is_some_and(|text| text.lines().any(|line| line.starts_with("来源：")));
    let preview = if is_markdown(path) && !is_artifact {
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
    let description_text = description.as_ref().map(|text| {
        text.lines()
            .filter(|line| !line.starts_with('#') && !line.starts_with("来源："))
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string()
    });
    Ok(LibraryItem {
        relative_path,
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        folder,
        is_markdown: is_markdown(path),
        is_artifact,
        preview,
        modified: modified.to_rfc3339(),
        source: description.and_then(|text| {
            text.lines()
                .find_map(|line| line.strip_prefix("来源："))
                .filter(|value| *value != "待确认")
                .map(str::to_string)
        }),
        description: description_text,
    })
}

fn build_index(root: &Path) -> Result<SearchIndex, String> {
    initialize(root)?;
    let mut rows = Vec::new();
    for category in CATEGORIES {
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
                let path = entry.path();
                if category == "Docs" && is_markdown(&path) {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if let Some(artifact_name) = name.strip_suffix(".md") {
                        if fs::symlink_metadata(folder.join(artifact_name))
                            .is_ok_and(|meta| meta.file_type().is_file())
                            && read_regular_text(&path)
                                .unwrap_or_default()
                                .lines()
                                .any(|line| line.starts_with("来源："))
                        {
                            continue;
                        }
                    }
                }
                let row = item(root, &path)?;
                let mut searchable = row.name.to_lowercase();
                if row.is_markdown {
                    searchable
                        .push_str(&fs::read_to_string(&path).unwrap_or_default().to_lowercase());
                }
                if row.is_artifact {
                    let sidecar = path.with_file_name(format!("{}.md", row.name));
                    searchable.push_str(
                        &read_regular_text(&sidecar)
                            .unwrap_or_default()
                            .to_lowercase(),
                    );
                }
                rows.push(IndexedItem {
                    item: row,
                    searchable,
                });
            }
        }
    }
    rows.sort_by(|a, b| {
        b.item
            .modified
            .cmp(&a.item.modified)
            .then_with(|| a.item.relative_path.cmp(&b.item.relative_path))
    });
    Ok(SearchIndex {
        root: root.to_path_buf(),
        dirty: false,
        items: rows,
    })
}

pub fn list(
    root: &Path,
    category: Option<&str>,
    query: Option<&str>,
) -> Result<Vec<LibraryItem>, String> {
    if category.is_some_and(|value| !CATEGORIES.contains(&value)) {
        return Err("Unknown category".into());
    }
    let needle = query.unwrap_or_default().trim().to_lowercase();
    let mut guard = index().lock().unwrap_or_else(|e| e.into_inner());
    if guard
        .as_ref()
        .is_none_or(|current| current.root != root || current.dirty)
    {
        *guard = Some(build_index(root)?);
    }
    Ok(guard
        .as_ref()
        .unwrap()
        .items
        .iter()
        .filter(|entry| {
            category.is_none_or(|value| entry.item.relative_path.starts_with(&format!("{value}/")))
        })
        .filter(|entry| needle.is_empty() || entry.searchable.contains(&needle))
        .map(|entry| entry.item.clone())
        .collect())
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

pub fn create_folder(root: &Path, folder: &str) -> Result<String, String> {
    initialize(root)?;
    let path = ensure_folder(root, folder)?;
    relative_string(root, &path)
}

fn trash_item_with(
    root: &Path,
    relative: &str,
    mut move_one: impl FnMut(&Path) -> Result<PathBuf, String>,
) -> Result<(), String> {
    let path = existing_path(root, relative)?;
    if !fs::metadata(&path).map_err(|e| e.to_string())?.is_file() {
        return Err("Only files can be moved to Trash".into());
    }
    let artifact = item(root, &path)?;
    let sidecar = path.with_file_name(format!("{}.md", artifact.name));
    let trashed_primary = move_one(&path)?;
    if artifact.is_artifact
        && fs::symlink_metadata(&sidecar).is_ok_and(|meta| meta.file_type().is_file())
    {
        if let Err(error) = move_one(&sidecar) {
            fs::rename(&trashed_primary, &path).map_err(|restore| {
                format!("Trash failed and restore failed: {error}; {restore}")
            })?;
            return Err(error);
        }
    }
    invalidate_index(root);
    Ok(())
}

#[cfg(target_os = "macos")]
fn move_to_system_trash(path: &Path) -> Result<PathBuf, String> {
    use objc2_foundation::{NSFileManager, NSString, NSURL};
    let text = path.to_str().ok_or("Invalid file path")?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(text));
    let mut resulting_url = None;
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, Some(&mut resulting_url))
        .map_err(|e| format!("System Trash failed: {e}"))?;
    let resulting_url = resulting_url.ok_or("System Trash did not return a destination")?;
    let resulting_path = resulting_url.path().ok_or("Invalid Trash destination")?;
    Ok(PathBuf::from(resulting_path.to_string()))
}

#[tauri::command]
pub fn sidebar_trash_item(relative_path: String) -> Result<(), String> {
    crate::sidebar_log::trace("trash_item", safe_log_path(&relative_path), || {
        let root = default_root()?;
        trash_item_with(&root, &relative_path, move_to_system_trash)
    })
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
    super::storage::atomic_create(&path, &mut content.as_bytes())?;
    invalidate_index(root);
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
    super::storage::atomic_create(&destination, &mut input)?;
    invalidate_index(root);
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
    static SAVE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = SAVE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|e| e.to_string())?;
    let path = existing_path(root, relative)?;
    if !is_markdown(&path) {
        return Err("Only Markdown files can be edited".into());
    }
    let previous = fs::read(&path).map_err(|e| e.to_string())?;
    if revision(&previous) != expected_revision {
        let copy = write_conflict_copy(&path, content.as_bytes())?;
        invalidate_index(root);
        return Ok(SaveOutcome::Conflict {
            copy: relative_string(root, &copy)?,
        });
    }
    #[cfg(target_os = "macos")]
    {
        // Reserve a visible draft before publication so every later failure
        // still leaves the user's edited version easy to find.
        let draft_copy = write_conflict_copy(&path, content.as_bytes())?;
        let displaced = match super::storage::atomic_swap(&path, content.as_bytes()) {
            Ok(path) => path,
            Err(error) => {
                let _ = fs::remove_file(draft_copy);
                return Err(error);
            }
        };
        if !fs::symlink_metadata(&displaced).is_ok_and(|meta| meta.file_type().is_file()) {
            expose_hidden_conflict(root, &path, &displaced)?;
            return Ok(SaveOutcome::Conflict {
                copy: relative_string(root, &draft_copy)?,
            });
        }
        let displaced_bytes = match fs::read(&displaced) {
            Ok(bytes) => bytes,
            Err(_) => {
                expose_hidden_conflict(root, &path, &displaced)?;
                invalidate_index(root);
                return Ok(SaveOutcome::Conflict {
                    copy: relative_string(root, &draft_copy)?,
                });
            }
        };
        if revision(&displaced_bytes) != expected_revision {
            // The external writer won after our first read. Keep the draft as a
            // visible conflict file, restore the displaced version atomically,
            // and also retain any third write that landed during restoration.
            let replaced = match super::storage::atomic_swap(&path, &displaced_bytes) {
                Ok(path) => path,
                Err(_) => {
                    expose_hidden_conflict(root, &path, &displaced)?;
                    invalidate_index(root);
                    return Ok(SaveOutcome::Conflict {
                        copy: relative_string(root, &draft_copy)?,
                    });
                }
            };
            let replaced_bytes = match fs::read(&replaced) {
                Ok(bytes) => bytes,
                Err(_) => {
                    expose_hidden_conflict(root, &path, &replaced)?;
                    let _ = fs::remove_file(displaced);
                    invalidate_index(root);
                    return Ok(SaveOutcome::Conflict {
                        copy: relative_string(root, &draft_copy)?,
                    });
                }
            };
            if replaced_bytes != content.as_bytes() {
                if write_conflict_copy(&path, &replaced_bytes).is_err() {
                    expose_hidden_conflict(root, &path, &replaced)?;
                } else {
                    let _ = fs::remove_file(&replaced);
                }
            } else {
                let _ = fs::remove_file(&replaced);
            }
            let _ = fs::remove_file(displaced);
            invalidate_index(root);
            return Ok(SaveOutcome::Conflict {
                copy: relative_string(root, &draft_copy)?,
            });
        }
        let _ = fs::remove_file(displaced);
        let _ = fs::remove_file(draft_copy);
    }
    #[cfg(not(target_os = "macos"))]
    super::storage::atomic_write(
        path.to_str().ok_or("Invalid note path")?,
        content.as_bytes(),
    )?;
    invalidate_index(root);
    Ok(SaveOutcome::Saved {
        revision: revision(content.as_bytes()),
    })
}

#[cfg(target_os = "macos")]
fn expose_hidden_conflict(root: &Path, original: &Path, hidden: &Path) -> Result<(), String> {
    let parent = original.parent().ok_or("Note has no parent folder")?;
    let stem = original.file_stem().unwrap_or_default().to_string_lossy();
    let visible = unique_path(parent, &format!("{stem}-external-conflict.md"));
    fs::rename(hidden, &visible).map_err(|_| {
        let relative = relative_string(root, hidden).unwrap_or_else(|_| "hidden conflict".into());
        format!("Could not expose external version; recover library-relative file {relative}")
    })?;
    invalidate_index(root);
    Ok(())
}

fn write_conflict_copy(path: &Path, data: &[u8]) -> Result<PathBuf, String> {
    let parent = path.parent().ok_or("Note has no parent folder")?;
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let name = format!(
        "{stem}-conflict-{}.md",
        Local::now().format("%Y%m%d-%H%M%S")
    );
    let copy = unique_path(parent, &name);
    let mut input = data;
    super::storage::atomic_create(&copy, &mut input)?;
    Ok(copy)
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
    super::storage::atomic_create(&sidecar, &mut body.as_bytes())?;
    invalidate_index(root);
    Ok(())
}

pub fn import_candidates(paths: &[PathBuf]) -> Result<Vec<ImportCandidate>, String> {
    let mut candidates = Vec::new();
    for source in paths {
        let metadata = fs::symlink_metadata(source).map_err(|e| e.to_string())?;
        if metadata.file_type().is_symlink() {
            return Err("Symlink imports are not allowed".into());
        }
        if metadata.is_file() {
            candidates.push(ImportCandidate {
                source_path: source.to_string_lossy().into_owned(),
                relative_folder: String::new(),
            });
            continue;
        }
        if !metadata.is_dir() {
            return Err("Import source must be a file or directory".into());
        }
        let top = source
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or("Invalid folder name")?;
        if top.starts_with('.') {
            return Err("Hidden folders cannot be imported".into());
        }
        let mut pending = vec![(source.to_path_buf(), top.to_string())];
        while let Some((directory, relative_folder)) = pending.pop() {
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
                    pending.push((entry.path(), format!("{relative_folder}/{name}")));
                } else if kind.is_file() {
                    candidates.push(ImportCandidate {
                        source_path: entry.path().to_string_lossy().into_owned(),
                        relative_folder: relative_folder.clone(),
                    });
                }
            }
        }
    }
    candidates.sort_by(|a, b| a.source_path.cmp(&b.source_path));
    Ok(candidates)
}

#[tauri::command]
pub fn sidebar_import_candidates(paths: Vec<String>) -> Result<Vec<ImportCandidate>, String> {
    crate::sidebar_log::trace("import_candidates", None, || {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        import_candidates(&paths)
    })
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
        if let Err(error) = write_description(root, &imported, description, source_url) {
            let _ = fs::remove_file(root.join(&imported.relative_path));
            return Err(error);
        }
        return Ok(vec![item(root, &root.join(imported.relative_path))?]);
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
                if let Err(error) = write_description(root, &item, description, source_url) {
                    let _ = fs::remove_file(root.join(&item.relative_path));
                    return Err(error);
                }
                imported.push(self::item(root, &root.join(item.relative_path))?);
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
        || create_folder(&default_root()?, &folder),
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
    crate::sidebar_log::trace("open_file", safe_log_path(&relative_path), || {
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
    })
}

#[tauri::command]
pub fn sidebar_show_in_finder(relative_path: String) -> Result<(), String> {
    crate::sidebar_log::trace("show_in_finder", safe_log_path(&relative_path), || {
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
    })
}

#[tauri::command]
pub fn sidebar_open_source(relative_path: String) -> Result<(), String> {
    crate::sidebar_log::trace("open_source", safe_log_path(&relative_path), || {
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
                        invalidate_index(&root);
                        let _ = crate::sidebar_log::trace("watch_event", None, || {
                            app.emit("sidebar-files-changed", ())
                                .map_err(|e| e.to_string())
                        });
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
