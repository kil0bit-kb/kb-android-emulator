// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use tauri::Manager;
use std::os::windows::process::CommandExt;

const DAEMON_BYTES: &[u8] = include_bytes!("../bin/fullscreen_daemon.exe");

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .setup(|app| {
            // Ensure android-sdk directories exist on startup
            let app_dir = app.path().app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            commands::ensure_dirs(&app_dir);

            // Write daemon to AppData directory if missing or size differs
            let daemon_path = app_dir.join("fullscreen_daemon.exe");
            let write_daemon = match std::fs::metadata(&daemon_path) {
                Ok(meta) => meta.len() != DAEMON_BYTES.len() as u64,
                Err(_) => true,
            };
            if write_daemon {
                let _ = std::fs::create_dir_all(&app_dir);
                let _ = std::fs::write(&daemon_path, DAEMON_BYTES);
            }

            // Kill any old instance first to avoid duplicates
            let _ = std::process::Command::new("taskkill")
                .args(["/F", "/IM", "fullscreen_daemon.exe"])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .output();

            // Spawn daemon detached with no console window
            let _ = std::process::Command::new(&daemon_path)
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .spawn();

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_system_info,
            commands::detect_gpus,
            commands::set_gpu_preference,
            commands::check_hypervisor,
            commands::enable_whpx,
            commands::check_install_status,
            commands::install_jdk,
            commands::install_cmdline_tools,
            commands::accept_licenses,
            commands::install_package,
            commands::list_avds,
            commands::create_avd,
            commands::delete_avd,
            commands::launch_avd,
            commands::stop_avd,
            commands::open_folder,
            commands::window_minimize,
            commands::window_maximize,
            commands::window_close,
            commands::fetch_sdk_packages,
            commands::update_avd_config,
            commands::optimize_guest_apps,
            commands::get_app_version,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                // Kill daemon when Tauri exits
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/IM", "fullscreen_daemon.exe"])
                    .creation_flags(0x08000000) // CREATE_NO_WINDOW
                    .output();
            }
        });
}
