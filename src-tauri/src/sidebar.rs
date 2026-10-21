use crate::commands::sidebar_library;
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Position, Size, WebviewUrl,
    WebviewWindowBuilder,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct SidebarState {
    pub side: String,
    pub mode: String,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            side: "right".into(),
            mode: "collapsed".into(),
        }
    }
}

fn state() -> &'static Mutex<SidebarState> {
    static STATE: OnceLock<Mutex<SidebarState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(load_state()))
}

fn state_path() -> Result<std::path::PathBuf, String> {
    Ok(crate::commands::paths::config_dir()?.join("sidebar-window.json"))
}

fn load_state() -> SidebarState {
    state_path()
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_state(value: &SidebarState) -> Result<(), String> {
    let path = state_path()?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_vec(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    std::fs::rename(temp, path).map_err(|e| e.to_string())
}

fn width(mode: &str) -> Result<f64, String> {
    match mode {
        "collapsed" => Ok(52.0),
        "normal" => Ok(320.0),
        "expanded" => Ok(720.0),
        _ => Err("Unknown sidebar mode".into()),
    }
}

fn geometry(app: &AppHandle, value: &SidebarState) -> Result<(f64, f64, f64, f64), String> {
    let monitor = app
        .primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or("No primary display")?;
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    let w = width(&value.mode)?;
    let full_height = area.size.height as f64 / scale;
    let h = if value.mode == "collapsed" {
        full_height.min(300.0)
    } else {
        full_height
    };
    let x = area.position.x as f64 / scale
        + if value.side == "left" {
            0.0
        } else {
            area.size.width as f64 / scale - w
        };
    let y = area.position.y as f64 / scale + (full_height - h) / 2.0;
    Ok((x, y, w, h))
}

pub fn show(app: &AppHandle) -> Result<(), String> {
    let root = sidebar_library::default_root()?;
    sidebar_library::initialize(&root)?;
    if let Some(window) = app.get_webview_window("sidebar") {
        window.show().map_err(|e| e.to_string())?;
        return Ok(());
    }
    let value = state().lock().unwrap_or_else(|e| e.into_inner()).clone();
    let (x, y, w, h) = geometry(app, &value)?;
    WebviewWindowBuilder::new(
        app,
        "sidebar",
        WebviewUrl::App("index.html?window=sidebar".into()),
    )
    .title("Sidebar Notes")
    .inner_size(w, h)
    .position(x, y)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .visible_on_all_workspaces(true)
    .skip_taskbar(true)
    .focused(false)
    .build()
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_mode(app: &AppHandle, mode: &str) -> Result<(), String> {
    width(mode)?;
    let mut value = state().lock().unwrap_or_else(|e| e.into_inner());
    value.mode = mode.to_string();
    apply(app, &value)?;
    save_state(&value)
}

pub fn set_side(app: &AppHandle, side: &str) -> Result<(), String> {
    if side != "left" && side != "right" {
        return Err("Unknown sidebar side".into());
    }
    let mut value = state().lock().unwrap_or_else(|e| e.into_inner());
    value.side = side.to_string();
    apply(app, &value)?;
    save_state(&value)
}

fn apply(app: &AppHandle, value: &SidebarState) -> Result<(), String> {
    let window = app
        .get_webview_window("sidebar")
        .ok_or("Sidebar is not open")?;
    let (x, y, w, h) = geometry(app, value)?;
    window
        .set_size(Size::Logical(LogicalSize::new(w, h)))
        .map_err(|e| e.to_string())?;
    window
        .set_position(Position::Logical(LogicalPosition::new(x, y)))
        .map_err(|e| e.to_string())?;
    let _ = window.emit("sidebar-state", value);
    Ok(())
}

pub fn toggle_expanded(app: &AppHandle) -> Result<(), String> {
    let current = state()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .mode
        .clone();
    set_mode(
        app,
        if current == "collapsed" {
            "normal"
        } else {
            "collapsed"
        },
    )
}

#[tauri::command]
pub fn sidebar_get_state() -> SidebarState {
    state().lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[tauri::command]
pub fn sidebar_set_mode(app: AppHandle, mode: String) -> Result<(), String> {
    set_mode(&app, &mode)
}

#[tauri::command]
pub fn sidebar_set_side(app: AppHandle, side: String) -> Result<(), String> {
    set_side(&app, &side)
}

#[tauri::command]
pub fn sidebar_show(app: AppHandle) -> Result<(), String> {
    show(&app)
}
