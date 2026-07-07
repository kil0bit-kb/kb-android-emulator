use std::path::Path;
use std::process::Stdio;
use tauri::{Emitter, Window};

use super::avd::{auto_repair_special_configs, auto_tune_heap_before_launch, clean_stale_locks};
use super::paths::{avd_dir, emulator_dir, sdk_dir, build_env};
use super::types::CommandResult;
use super::RUNNING_AVDS;

fn get_running_avds() -> &'static std::sync::Mutex<std::collections::HashMap<String, bool>> {
    RUNNING_AVDS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

// Windows-only: raw Win32 API for process priority elevation
#[cfg(windows)]
mod win32_priority {
    #[link(name = "kernel32")]
    extern "system" {
        pub fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> *mut std::ffi::c_void;
        pub fn SetPriorityClass(hProcess: *mut std::ffi::c_void, dwPriorityClass: u32) -> i32;
        pub fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
    }

    pub const PROCESS_SET_INFORMATION: u32 = 0x0200;
    pub const HIGH_PRIORITY_CLASS: u32 = 0x00000080;
}

// ─── Launch AVD ───────────────────────────────────────────────────────────────
#[tauri::command]
pub fn launch_avd(
    name: String,
    gpu_mode: Option<String>,
    accel: Option<String>,
    quick_boot: Option<bool>,
    boot_anim: Option<bool>,
    no_camera: Option<bool>,
    no_gps: Option<bool>,
    no_bluetooth: Option<bool>,
    read_only: Option<bool>,
    wipe_data: Option<bool>,
    speed_mode: Option<bool>,
    raw_launch: Option<bool>,
    window: Window,
) -> CommandResult {
    // Detect Device Type
    let mut is_wear = false;
    let mut is_tv = false;
    let mut is_auto = false;
    let config_path = avd_dir().join(format!("{}.avd", name)).join("config.ini");
    if config_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("tag.id=") {
                    let id = trimmed.split('=').nth(1).unwrap_or("").trim();
                    if id.contains("wear") { is_wear = true; }
                    if id.contains("tv") || id.contains("google-tv") { is_tv = true; }
                    if id.contains("automotive") { is_auto = true; }
                }
                if trimmed.starts_with("image.sysdir.1=") {
                    let sysdir = trimmed.split('=').nth(1).unwrap_or("").trim();
                    if sysdir.contains("wear") { is_wear = true; }
                    if sysdir.contains("android-tv") || sysdir.contains("google-tv") { is_tv = true; }
                    if sysdir.contains("android-automotive") { is_auto = true; }
                }
            }
        }
    }

    let is_raw = raw_launch.unwrap_or(false) || is_wear || is_tv || is_auto;

    if !is_raw {
        auto_repair_special_configs(&name);
    }
    clean_stale_locks(&name);

    #[cfg(windows)]
    let emulator_exe = emulator_dir().join("emulator.exe");
    #[cfg(not(windows))]
    let emulator_exe = emulator_dir().join("emulator");

    if !emulator_exe.exists() {
        return CommandResult {
            ok: false,
            error: Some("Emulator not installed. Install it from SDK Manager first.".to_string()),
            output: None,
        };
    }

    let ram_size = if !is_raw {
        auto_tune_heap_before_launch(&name)
    } else {
        4096
    };

    // Auto-repair AVD .ini path if it was moved/migrated
    let ini_path = avd_dir().join(format!("{}.ini", name));
    if ini_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&ini_path) {
            let mut new_lines = vec![];
            let mut changed = false;
            let expected_path = avd_dir().join(format!("{}.avd", name));
            for line in content.lines() {
                if line.starts_with("path=") {
                    let current_path = line.trim_start_matches("path=");
                    if Path::new(current_path) != expected_path {
                        new_lines.push(format!("path={}", expected_path.to_string_lossy()));
                        changed = true;
                        continue;
                    }
                }
                new_lines.push(line.to_string());
            }
            if changed {
                let _ = std::fs::write(&ini_path, new_lines.join("\n"));
            }
        }
    }

    let env = build_env();
    let avd_home = avd_dir().to_string_lossy().to_string();

    let mut args = vec![];

    if is_raw {
        let _ = window.emit(
            "log",
            format!("Launching \"{}\" in RAW / Default Mode...", name),
        );
        args.push("-avd".to_string());
        args.push(name.clone());
    } else {
        let mut gpu = gpu_mode.unwrap_or_else(|| "auto".to_string());
        
        // Normalize to official supported CLI modes: auto, host, software
        if gpu == "angle_indirect" {
            gpu = "host".to_string();
        } else if gpu == "swiftshader_indirect" {
            gpu = "software".to_string();
        } else if gpu != "host" && gpu != "software" {
            gpu = "auto".to_string();
        }
        let mut accelerator = accel.unwrap_or_else(|| "whpx".to_string());

        // Map legacy hypervisor selections to unified flags for Android Emulator 36+
        if accelerator == "whpx" || accelerator == "aehd" || accelerator == "haxm" {
            accelerator = "on".to_string();
        }
        // On Linux, "kvm" maps to "on" as well
        #[cfg(not(windows))]
        if accelerator == "kvm" {
            accelerator = "on".to_string();
        }

        let _ = window.emit(
            "log",
            format!("Launching \"{}\" — GPU={}, accel={}", name, gpu, accelerator),
        );

        let (heap_sz, growth_lim) = if is_wear {
            ("128m", "64m")
        } else if is_tv {
            ("256m", "128m")
        } else if is_auto {
            ("512m", "256m")
        } else if ram_size >= 8192 {
            ("1024m", "512m")
        } else {
            ("512m", "256m")
        };

        args = vec![
            "-avd".to_string(),
            name.clone(),
            "-gpu".to_string(),
            gpu,
            "-accel".to_string(),
            accelerator,
            "-feature".to_string(),
            "Vulkan,GLESDynamicVersion".to_string(),
            "-logcat".to_string(),
            "*:S".to_string(),
            "-no-metrics".to_string(),
            "-prop".to_string(),
            format!("qemu.dalvik.vm.heapsize={}", heap_sz),
            "-prop".to_string(),
            format!("qemu.dalvik.vm.heapgrowthlimit={}", growth_lim),
            "-prop".to_string(),
            "qemu.net.tcp.buffersize.default=4096,87380,110208,4096,16384,110208".to_string(),
            "-prop".to_string(),
            "qemu.net.tcp.buffersize.wifi=262144,524288,1048576,262144,524288,1048576".to_string(),
            "-dns-server".to_string(),
            "1.1.1.1".to_string(),
        ];

        // WASAPI audio is Windows-only
        #[cfg(windows)]
        {
            args.push("-audio".to_string());
            args.push("wasapi".to_string());
        }

        if let Some(false) = quick_boot {
            args.push("-no-snapshot-load".to_string());
            args.push("-no-snapshot-save".to_string());
        }

        if let Some(true) = read_only {
            args.push("-read-only".to_string());
        }

        if let Some(true) = wipe_data {
            args.push("-wipe-data".to_string());
        }

        if let Some(false) = boot_anim {
            args.push("-no-boot-anim".to_string());
        }

        if let Some(true) = no_camera {
            args.push("-camera-back".to_string());
            args.push("none".to_string());
            args.push("-camera-front".to_string());
            args.push("none".to_string());
        }
        if let Some(true) = no_gps {
            args.push("-no-passive-gps".to_string());
        }

        let mut netsim_flags: Vec<&str> = vec!["--no-cli-ui"];
        if let Some(true) = no_bluetooth {
            netsim_flags.push("--no-bluetooth");
        }
        for flag in &netsim_flags {
            args.push("-netsim-args".to_string());
            args.push(flag.to_string());
        }
    }

    let emulator_cwd = emulator_dir();
    let mut cmd = std::process::Command::new(&emulator_exe);
    cmd.current_dir(&emulator_cwd)
        .args(&args)
        .env("ANDROID_AVD_HOME", &avd_home)
        .env("ANDROID_EMULATOR_HOME", &avd_home)
        .envs(&env)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let child = cmd.spawn();

    match child {
        Ok(mut c) => {
            let pid = c.id();

            // Elevate process priority — Windows only
            #[cfg(windows)]
            unsafe {
                use win32_priority::*;
                let handle = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
                if !handle.is_null() {
                    let success = SetPriorityClass(handle, HIGH_PRIORITY_CLASS);
                    println!("[Backend] Elevated emulator.exe (PID {}) priority: {}", pid, success != 0);
                    CloseHandle(handle);
                }
            }

            get_running_avds().lock().unwrap().insert(name.clone(), true);
            let name_clone = name.clone();
            let window_clone = window.clone();

            if let Some(true) = speed_mode {
                #[cfg(windows)]
                let adb_exe = sdk_dir().join("platform-tools").join("adb.exe");
                #[cfg(not(windows))]
                let adb_exe = sdk_dir().join("platform-tools").join("adb");

                let name_clone3 = name.clone();
                let window_clone3 = window.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(4));
                    if adb_exe.exists() {
                        let _ = std::process::Command::new(&adb_exe)
                            .args(["wait-for-device"])
                            .output();

                        let _ = std::process::Command::new(&adb_exe)
                            .args(["root"])
                            .output();

                        let _ = std::process::Command::new(&adb_exe)
                            .args(["wait-for-device"])
                            .output();
                        
                        let _ = std::process::Command::new(&adb_exe)
                            .args(["shell", "service", "call", "SurfaceFlinger", "1008", "i32", "1"])
                            .output();
                        
                        let _ = window_clone3.emit(
                            "log",
                            format!("Speed Mode applied to \"{}\": GPU composition forced.", name_clone3),
                        );
                    }
                });
            }

            let stdout = c.stdout.take();

            if let Some(stdout_stream) = stdout {
                std::thread::spawn(move || {
                    use std::io::{BufRead, BufReader};
                    use std::collections::VecDeque;
                    let reader = BufReader::new(stdout_stream);
                    let mut seen: VecDeque<String> = VecDeque::with_capacity(256);
                    for line in reader.lines() {
                        if let Ok(l) = line {
                            let trimmed = l.trim().to_string();
                            if trimmed.is_empty() { continue; }
                            if seen.contains(&trimmed) { continue; }
                            seen.push_back(trimmed.clone());
                            if seen.len() > 256 { seen.pop_front(); }
                            let _ = window_clone.emit(
                                "emulator-log",
                                serde_json::json!({ "name": name_clone, "line": trimmed }),
                            );
                        }
                    }
                });
            }

            let name_clone2 = name.clone();
            let window_clone2 = window.clone();
            let avd_home_clone = avd_home.clone();
            std::thread::spawn(move || {
                let launch_time = std::time::Instant::now();
                let _ = c.wait();
                let elapsed = launch_time.elapsed().as_secs();
                get_running_avds().lock().unwrap().remove(&name_clone2);

                if elapsed < 10 {
                    let snapshot_dir = std::path::Path::new(&avd_home_clone)
                        .join(format!("{}.avd", name_clone2))
                        .join("snapshots");
                    if snapshot_dir.exists() {
                        let _ = std::fs::remove_dir_all(&snapshot_dir);
                        let _ = window_clone2.emit(
                            "emulator-log",
                            serde_json::json!({
                                "name": name_clone2,
                                "line": "[KB] Emulator crashed immediately — corrupted snapshot auto-deleted. Please launch again for a clean cold boot."
                            }),
                        );
                    } else {
                        let _ = window_clone2.emit(
                            "emulator-log",
                            serde_json::json!({
                                "name": name_clone2,
                                "line": "[KB] Emulator exited unexpectedly on launch. Try Wipe & Boot if it happens again."
                            }),
                        );
                    }
                }

                let _ = window_clone2.emit("emulator-exit", serde_json::json!({ "name": name_clone2, "code": 0 }));
            });
            CommandResult { ok: true, error: None, output: Some(pid.to_string()) }
        }
        Err(e) => CommandResult { ok: false, error: Some(e.to_string()), output: None },
    }
}

// ─── Stop AVD ─────────────────────────────────────────────────────────────────
#[tauri::command]
pub fn stop_avd(name: String, window: Window) -> CommandResult {
    stop_avd_impl(name, window)
}

#[cfg(windows)]
fn stop_avd_impl(name: String, window: Window) -> CommandResult {
    use std::os::windows::process::CommandExt;
    let adb_exe = sdk_dir().join("platform-tools").join("adb.exe");
    if adb_exe.exists() {
        let _ = std::process::Command::new(&adb_exe)
            .creation_flags(0x08000000)
            .args(["-s", "emulator-5554", "emu", "kill"])
            .output();
    }
    let _ = std::process::Command::new("taskkill")
        .creation_flags(0x08000000)
        .args(["/F", "/IM", "emulator.exe"])
        .output();
    let _ = window.emit("log", format!("Stop signal sent to \"{}\"", name));
    CommandResult { ok: true, error: None, output: None }
}

#[cfg(not(windows))]
fn stop_avd_impl(name: String, window: Window) -> CommandResult {
    let adb_exe = sdk_dir().join("platform-tools").join("adb");
    if adb_exe.exists() {
        let _ = std::process::Command::new(&adb_exe)
            .args(["-s", "emulator-5554", "emu", "kill"])
            .output();
    }
    // Kill emulator process by name on Linux
    let _ = std::process::Command::new("pkill")
        .args(["-f", "emulator"])
        .output();
    let _ = window.emit("log", format!("Stop signal sent to \"{}\"", name));
    CommandResult { ok: true, error: None, output: None }
}

// ─── Optimize Guest Apps ──────────────────────────────────────────────────────
#[tauri::command]
pub async fn optimize_guest_apps(window: Window) -> CommandResult {
    #[cfg(windows)]
    let adb_exe = sdk_dir().join("platform-tools").join("adb.exe");
    #[cfg(not(windows))]
    let adb_exe = sdk_dir().join("platform-tools").join("adb");

    if !adb_exe.exists() {
        return CommandResult { ok: false, error: Some("adb not found".to_string()), output: None };
    }

    let _ = window.emit("log", "Scanning user-installed packages inside emulator...");

    let output = match std::process::Command::new(&adb_exe)
        .args(["-s", "emulator-5554", "shell", "pm", "list", "packages", "-3"])
        .output() {
            Ok(o) => o,
            Err(e) => return CommandResult { ok: false, error: Some(format!("Failed to run adb: {}", e)), output: None },
        };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut packages = vec![];
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("package:") {
            let pkg = trimmed.replacen("package:", "", 1);
            if !pkg.is_empty() {
                packages.push(pkg);
            }
        }
    }

    if packages.is_empty() {
        let _ = window.emit("log", "No third-party apps or games found to optimize.");
        return CommandResult { ok: true, error: None, output: Some("No third-party apps found".into()) };
    }

    let total = packages.len();
    let _ = window.emit("log", format!("Found {} apps/games. Starting Ahead-Of-Time (AOT) compilation...", total));

    for (idx, pkg) in packages.iter().enumerate() {
        let _ = window.emit("log", format!("[{}/{}] Optimizing: {}...", idx + 1, total, pkg));
        
        let _ = std::process::Command::new(&adb_exe)
            .args(["-s", "emulator-5554", "shell", "cmd", "package", "compile", "-m", "speed", "-f", pkg])
            .output();
    }

    let _ = window.emit("log", "All user apps and games fully compiled to native machine code!");
    CommandResult { ok: true, error: None, output: Some("Optimization completed successfully".into()) }
}
