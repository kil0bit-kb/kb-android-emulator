// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use tauri::Manager;

// The fullscreen daemon is Windows-only
#[cfg(windows)]
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

            // Windows-only: write daemon to AppData directory and launch it
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;

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
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::system_info::get_system_info,
            commands::system_info::detect_gpus,
            commands::hypervisor::set_gpu_preference,
            commands::hypervisor::check_hypervisor,
            commands::hypervisor::enable_whpx,
            commands::install::check_install_status,
            commands::install::install_jdk,
            commands::install::install_cmdline_tools,
            commands::install::accept_licenses,
            commands::install::install_package,
            commands::install::uninstall_package,
            commands::avd::list_avds,
            commands::avd::create_avd,
            commands::avd::delete_avd,
            commands::launch::launch_avd,
            commands::launch::stop_avd,
            commands::window::open_folder,
            commands::window::window_minimize,
            commands::window::window_maximize,
            commands::window::window_close,
            commands::install::fetch_sdk_packages,
            commands::install::cancel_download,
            commands::avd::update_avd_config,
            commands::launch::optimize_guest_apps,
            commands::install::uninstall_jdk,
            commands::install::uninstall_cmdline_tools,
            commands::window::get_app_version,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                // Kill daemon when Tauri exits — Windows only
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    let _ = std::process::Command::new("taskkill")
                        .args(["/F", "/IM", "fullscreen_daemon.exe"])
                        .creation_flags(0x08000000) // CREATE_NO_WINDOW
                        .output();
                }
            }
        });
}
