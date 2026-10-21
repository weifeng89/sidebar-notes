//! Redacted, local diagnostics for Sidebar Notes operations.
use chrono::{Duration, Local, NaiveDate};
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

#[derive(Serialize)]
struct Record<'a> {
    at: String,
    operation_id: &'a str,
    operation: &'a str,
    phase: &'a str,
    duration_ms: u128,
    result: &'a str,
    error_code: Option<&'a str>,
    relative_path: Option<&'a str>,
}

fn lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn append_at(directory: &Path, record: &Record<'_>) -> Result<(), String> {
    let _guard = lock().lock().unwrap_or_else(|e| e.into_inner());
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let cutoff = Local::now().date_naive() - Duration::days(14);
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if let Some(date) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_suffix(".jsonl"))
            .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
        {
            if date < cutoff {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    let path = directory.join(format!("{}.jsonl", Local::now().format("%Y-%m-%d")));
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut file, record).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())
}

pub fn trace<T>(
    operation: &'static str,
    relative_path: Option<&str>,
    action: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let start = Instant::now();
    let directory = crate::commands::paths::config_dir()
        .ok()
        .map(|path| path.join("sidebar-logs"));
    let make = |phase, result, error_code| Record {
        at: Local::now().to_rfc3339(),
        operation_id: &id,
        operation,
        phase,
        duration_ms: start.elapsed().as_millis(),
        result,
        error_code,
        relative_path,
    };
    if let Some(dir) = &directory {
        let _ = append_at(dir, &make("start", "pending", None));
    }
    let result = action();
    if let Some(dir) = &directory {
        let code = if result.is_err() {
            Some("operation_failed")
        } else {
            None
        };
        let _ = append_at(
            dir,
            &make(
                "complete",
                if result.is_ok() { "ok" } else { "error" },
                code,
            ),
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn records_only_structured_metadata_and_rotates_old_days() {
        let temp = tempdir().unwrap();
        let old = (Local::now().date_naive() - Duration::days(15))
            .format("%Y-%m-%d")
            .to_string();
        fs::write(temp.path().join(format!("{old}.jsonl")), "old").unwrap();
        let record = Record {
            at: Local::now().to_rfc3339(),
            operation_id: "op-1",
            operation: "import",
            phase: "complete",
            duration_ms: 12,
            result: "ok",
            error_code: None,
            relative_path: Some("Docs/GTM"),
        };
        append_at(temp.path(), &record).unwrap();
        assert!(!temp.path().join(format!("{old}.jsonl")).exists());
        let today = temp
            .path()
            .join(format!("{}.jsonl", Local::now().format("%Y-%m-%d")));
        let text = fs::read_to_string(today).unwrap();
        assert!(text.contains("\"operation_id\":\"op-1\""));
        assert!(text.contains("\"relative_path\":\"Docs/GTM\""));
        assert!(!text.contains("/Users/"));
    }
}
