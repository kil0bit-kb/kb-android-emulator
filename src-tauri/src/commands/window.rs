use tauri::Window;

use super::types::CommandResult;

// ─── Window controls ──────────────────────────────────────────────────────────
#[tauri::command]
pub fn window_minimize(window: Window) {
    let _ = window.minimize();
}

#[tauri::command]
pub fn window_maximize(window: Window) {
    if window.is_maximized().unwrap_or(false) {
        let _ = window.unmaximize();
    } else {
        let _ = window.maximize();
    }
}

#[tauri::command]
pub fn window_close(window: Window) {
    let _ = window.close();
}

// ─── Open Folder ─────────────────────────────────────────────────────────────
#[tauri::command]
pub fn open_folder(folder_path: String) -> CommandResult {
    open_folder_impl(folder_path)
}

#[cfg(windows)]
fn open_folder_impl(folder_path: String) -> CommandResult {
    let _ = std::process::Command::new("explorer.exe")
        .arg(&folder_path)
        .spawn();
    CommandResult { ok: true, error: None, output: None }
}

#[cfg(target_os = "linux")]
fn open_folder_impl(folder_path: String) -> CommandResult {
    let _ = std::process::Command::new("xdg-open")
        .arg(&folder_path)
        .spawn();
    CommandResult { ok: true, error: None, output: None }
}

#[cfg(target_os = "macos")]
fn open_folder_impl(folder_path: String) -> CommandResult {
    let _ = std::process::Command::new("open")
        .arg(&folder_path)
        .spawn();
    CommandResult { ok: true, error: None, output: None }
}

// ─── App Version ─────────────────────────────────────────────────────────────
#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
