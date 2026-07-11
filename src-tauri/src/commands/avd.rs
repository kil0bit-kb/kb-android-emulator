use tauri::{Emitter, Window};

#[cfg(windows)]
#[allow(unused_imports)]
use std::os::windows::process::CommandExt;

use super::paths::{avd_dir, cmdline_dir, build_env};
use super::types::{AvdInfo, CommandResult, CreateAvdOptions};
use super::RUNNING_AVDS;
use std::process::Stdio;

fn get_running_avds() -> &'static std::sync::Mutex<std::collections::HashMap<String, bool>> {
    RUNNING_AVDS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

// ─── List AVDs ────────────────────────────────────────────────────────────────
#[tauri::command]
pub fn list_avds() -> Vec<AvdInfo> {
    let avd = avd_dir();
    if !avd.exists() { return vec![]; }

    let entries = match std::fs::read_dir(&avd) {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let mut avds = vec![];
    for entry in entries.filter_map(|e| e.ok()) {
        let fname = entry.file_name().to_string_lossy().to_string();
        if !fname.ends_with(".ini") || fname.contains("hardware") || fname == "emu-update-last-check.ini" { continue; }

        let name = fname.trim_end_matches(".ini").to_string();
        let ini_path = avd.join(&fname);
        let config_path = avd.join(format!("{}.avd", name)).join("config.ini");

        let ini_content = std::fs::read_to_string(&ini_path).unwrap_or_default();
        let config_content = std::fs::read_to_string(&config_path).unwrap_or_default();

        let get_val = |content: &str, key: &str| -> String {
            for line in content.lines() {
                if let Some(rest) = line.strip_prefix(&format!("{}=", key)) {
                    return rest.trim().to_string();
                }
            }
            String::new()
        };

        let is_running = {
            let map = get_running_avds().lock().unwrap();
            map.get(&name).copied().unwrap_or(false)
        };

        avds.push(AvdInfo {
            name,
            path: get_val(&ini_content, "path"),
            target: get_val(&config_content, "tag.display"),
            api: get_val(&config_content, "image.sysdir.1"),
            ram: get_val(&config_content, "hw.ramSize"),
            cores: get_val(&config_content, "hw.cpu.ncore"),
            gpu: get_val(&config_content, "hw.gpu.mode"),
            running: is_running,
        });
    }
    avds
}

// ─── Create AVD ───────────────────────────────────────────────────────────────
#[tauri::command]
pub async fn create_avd(options: CreateAvdOptions, window: Window) -> CommandResult {
    #[cfg(windows)]
    let bat = cmdline_dir().join("bin").join("avdmanager.bat");
    #[cfg(not(windows))]
    let bat = cmdline_dir().join("bin").join("avdmanager");

    let env = build_env();

    // Sanitize the AVD name (replace spaces with underscores)
    let sanitized_name = options.name.replace(" ", "_");

    let _ = window.emit("log", format!("Creating AVD: {} (sanitized as: {})", options.name, sanitized_name));

    #[cfg(windows)]
    let cmd_str = format!(
        "echo no | & '{}' create avd --name '{}' --package '{}' --force",
        bat.display(),
        sanitized_name,
        options.system_image
    );

    #[cfg(windows)]
    let mut cmd = {
        let mut c = tokio::process::Command::new("powershell");
        c.creation_flags(0x08000000);
        c.args(["-NoProfile", "-Command", &cmd_str]);
        c
    };

    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = tokio::process::Command::new(&bat);
        c.args(["create", "avd", "--name", &sanitized_name, "--package", &options.system_image, "--force"]);
        c
    };

    cmd.envs(&env)
       .current_dir(cmdline_dir())
       .stdin(Stdio::null())
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return CommandResult { ok: false, error: Some(e.to_string()), output: None },
    };

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => return CommandResult { ok: false, error: Some("Failed to open stdout".to_string()), output: None },
    };
    let stderr = match child.stderr.take() {
        Some(s) => s,
        None => return CommandResult { ok: false, error: Some("Failed to open stderr".to_string()), output: None },
    };

    use tokio::io::{AsyncBufReadExt, BufReader};
    let window_clone = window.clone();
    let stdout_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if !line.trim().is_empty() {
                let _ = window_clone.emit("log", line);
            }
        }
    });

    let window_clone_err = window.clone();
    let stderr_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if !line.trim().is_empty() {
                let _ = window_clone_err.emit("log", format!("[warn] {}", line));
            }
        }
    });

    let status = match child.wait().await {
        Ok(s) => s,
        Err(e) => return CommandResult { ok: false, error: Some(e.to_string()), output: None },
    };

    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    if !status.success() {
        return CommandResult {
            ok: false,
            error: Some(format!("avdmanager exited with {:?}", status.code())),
            output: None,
        };
    }

    // Apply high-performance config.ini tuning
    let config_path = avd_dir()
        .join(format!("{}.avd", sanitized_name))
        .join("config.ini");

    let is_special_device = options.system_image.contains("wear")
        || options.system_image.contains("android-tv")
        || options.system_image.contains("google-tv")
        || options.system_image.contains("android-automotive")
        || options.system_image.contains("automotive");

    if !is_special_device && !options.raw_launch.unwrap_or(false) && config_path.exists() {
        let mut content = std::fs::read_to_string(&config_path).unwrap_or_default();
        let perf_settings = build_perf_config(&options);
        for (k, v) in &perf_settings {
            let pattern = format!("{}=", k);
            if let Some(line_start) = content.find(&pattern) {
                let line_end = content[line_start..].find('\n').map(|i| i + line_start).unwrap_or(content.len());
                content.replace_range(line_start..line_end, &format!("{}={}", k, v));
            } else {
                content.push_str(&format!("\n{}={}", k, v));
            }
        }
        let _ = std::fs::write(&config_path, &content);
    }

    let _ = window.emit("log", format!("AVD \"{}\" created successfully with high-performance config!", sanitized_name));
    CommandResult { ok: true, error: None, output: None }
}

pub fn build_perf_config(opts: &CreateAvdOptions) -> Vec<(String, String)> {
    let (w, h) = opts.screen_resolution.split_once('x').unwrap_or(("1280", "720"));
    
    let heap_size = if opts.ram >= 8192 { "1024" } else { "512" };

    vec![
        ("hw.cpu.ncore".into(), opts.cores.to_string()),
        ("hw.ramSize".into(), opts.ram.to_string()),
        ("hw.gpu.mode".into(), opts.gpu_mode.clone()),
        ("hw.gpu.enabled".into(), "yes".into()),
        ("hw.audioInput".into(), "yes".into()),
        ("hw.keyboard".into(), "yes".into()),
        ("hw.mainKeys".into(), "no".into()),
        ("hw.lcd.width".into(), w.to_string()),
        ("hw.lcd.height".into(), h.to_string()),
        ("hw.lcd.density".into(), opts.dpi.to_string()),
        ("hw.sdCard".into(), "no".into()),
        ("disk.dataPartition.size".into(), format!("{}M", opts.storage)),
        ("hw.accelerometer".into(), "yes".into()),
        ("hw.gps".into(), "yes".into()),
        ("hw.sensors.orientation".into(), "yes".into()),
        ("fastboot.forceColdBoot".into(), "no".into()),
        ("fastboot.forceChosenSnapshotBoot".into(), "no".into()),
        ("fastboot.forceFastBoot".into(), "yes".into()),
        ("vm.heapSize".into(), heap_size.into()),
        ("disk.cachePartition".into(), "yes".into()),
        ("disk.cachePartition.size".into(), "800M".into()),
        ("hw.camera.back".into(), "virtualscene".into()),
        ("hw.camera.front".into(), "emulated".into()),
    ]
}

// ─── Delete AVD ───────────────────────────────────────────────────────────────
#[tauri::command]
pub async fn delete_avd(name: String, window: Window) -> CommandResult {
    #[cfg(windows)]
    let bat = cmdline_dir().join("bin").join("avdmanager.bat");
    #[cfg(not(windows))]
    let bat = cmdline_dir().join("bin").join("avdmanager");

    let env = build_env();

    #[cfg(windows)]
    let cmd_str = format!("& '{}' delete avd --name '{}'", bat.display(), name);

    #[cfg(windows)]
    let mut cmd = {
        let mut c = tokio::process::Command::new("powershell");
        c.creation_flags(0x08000000);
        c.args(["-NoProfile", "-Command", &cmd_str]);
        c
    };

    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = tokio::process::Command::new(&bat);
        c.args(["delete", "avd", "--name", &name]);
        c
    };

    cmd.envs(&env)
       .current_dir(cmdline_dir())
       .stdin(Stdio::null())
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());

    let child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return CommandResult { ok: false, error: Some(e.to_string()), output: None },
    };

    let out = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => return CommandResult { ok: false, error: Some(e.to_string()), output: None },
    };

    let _ = window.emit("log", format!("Deleted AVD: {}", name));
    CommandResult { ok: out.status.success(), error: None, output: None }
}

// ─── Auto-tune heap size before launch ────────────────────────────────────────
pub fn auto_tune_heap_before_launch(name: &str) -> u64 {
    let config_path = avd_dir().join(format!("{}.avd", name)).join("config.ini");
    if !config_path.exists() { return 4096; }
    
    let mut ram_val: u64 = 4096;
    let mut is_wear = false;
    let mut is_tv = false;
    let mut is_auto = false;
    
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
            if trimmed.starts_with("hw.ramSize=") {
                if let Some(val_str) = trimmed.split('=').nth(1) {
                    ram_val = val_str.parse::<u64>().unwrap_or(4096);
                }
            }
        }
        
        let target_heap = if is_wear {
            "128"
        } else if is_tv {
            "256"
        } else if is_auto {
            "512"
        } else if ram_val >= 8192 {
            "1024"
        } else {
            "512"
        };
        
        let is_special = is_wear || is_tv || is_auto;
        if !is_special {
            let mut heap_exists = false;
            let mut final_lines = vec![];
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("vm.heapSize=") {
                    final_lines.push(format!("vm.heapSize={}", target_heap));
                    heap_exists = true;
                } else {
                    final_lines.push(line.to_string());
                }
            }
            
            if !heap_exists {
                final_lines.push(format!("vm.heapSize={}", target_heap));
            }
            
            let _ = std::fs::write(&config_path, final_lines.join("\n"));
        }
    }
    ram_val
}

pub fn auto_repair_special_configs(name: &str) {
    let config_path = avd_dir().join(format!("{}.avd", name)).join("config.ini");
    if !config_path.exists() { return; }
    
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        let mut is_special = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("tag.id=") {
                let id = trimmed.split('=').nth(1).unwrap_or("").trim();
                if id.contains("wear") || id.contains("tv") || id.contains("google-tv") || id.contains("automotive") {
                    is_special = true;
                }
            }
            if trimmed.starts_with("image.sysdir.1=") {
                let sysdir = trimmed.split('=').nth(1).unwrap_or("").trim();
                if sysdir.contains("wear") || sysdir.contains("android-tv") || sysdir.contains("google-tv") || sysdir.contains("android-automotive") || sysdir.contains("automotive") {
                    is_special = true;
                }
            }
        }
        
        if is_special {
            let mut clean_lines = vec![];
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("hw.lcd.width=")
                    || trimmed.starts_with("hw.lcd.height=")
                    || trimmed.starts_with("hw.lcd.density=")
                    || trimmed.starts_with("skin.name=")
                    || trimmed.starts_with("skin.path=")
                    || trimmed.starts_with("hw.gpu.mode=")
                    || trimmed.starts_with("hw.gpu.enabled=")
                    || trimmed.starts_with("vm.heapSize=")
                {
                    continue;
                }
                clean_lines.push(line.to_string());
            }
            let _ = std::fs::write(&config_path, clean_lines.join("\n"));
        }
    }
}

pub fn clean_stale_locks(name: &str) {
    let avd_path = avd_dir().join(format!("{}.avd", name));
    if !avd_path.exists() { return; }

    let lock_file = avd_path.join("multiinstance.lock");
    if lock_file.exists() {
        let _ = std::fs::remove_file(&lock_file);
    }

    let lock_dir = avd_path.join("hardware-qemu.ini.lock");
    if lock_dir.exists() {
        let _ = std::fs::remove_dir_all(&lock_dir);
    }
}

// ─── Update AVD Config ────────────────────────────────────────────────────────
#[tauri::command]
pub fn update_avd_config(
    name: String,
    cores: String,
    ram: String,
    gpu: String,
    resolution: Option<String>,
    dpi: Option<String>,
) -> CommandResult {
    let config_path = avd_dir().join(format!("{}.avd", name)).join("config.ini");
    if !config_path.exists() {
        return CommandResult { ok: false, error: Some("AVD config.ini not found".to_string()), output: None };
    }
    
    let content = match std::fs::read_to_string(&config_path) {
        Ok(c) => c,
        Err(e) => return CommandResult { ok: false, error: Some(e.to_string()), output: None },
    };

    // Detect Device Type
    let mut is_wear = false;
    let mut is_tv = false;
    let mut is_auto = false;
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

    let is_special = is_wear || is_tv || is_auto;
    if is_special {
        return CommandResult { ok: true, error: None, output: None };
    }

    let user_cores = cores.parse::<u32>().unwrap_or(4);
    let user_ram = ram.parse::<u32>().unwrap_or(4096);

    let final_cores = if is_wear {
        user_cores.min(2).max(1)
    } else if is_tv {
        user_cores.min(2).max(1)
    } else if is_auto {
        user_cores.min(4).max(1)
    } else {
        user_cores
    };

    let final_ram = if is_wear {
        user_ram.min(1024).max(512)
    } else if is_tv {
        user_ram.min(2048).max(1024)
    } else if is_auto {
        user_ram.min(4096).max(2048)
    } else {
        user_ram
    };

    let heap_size = if is_wear {
        "128"
    } else if is_tv {
        "256"
    } else if is_auto {
        "512"
    } else if final_ram >= 8192 {
        "1024"
    } else {
        "512"
    };

    let mut new_lines = vec![];
    let mut updated_cores = false;
    let mut updated_ram = false;
    let mut updated_gpu = false;
    let mut updated_heap = false;
    let mut updated_width = false;
    let mut updated_height = false;
    let mut updated_density = false;
    let mut updated_skin = false;

    let (w, h) = if let Some(ref res) = resolution {
        res.split_once('x').unwrap_or(("1280", "720"))
    } else {
        ("1280", "720")
    };
    let density_val = dpi.clone().unwrap_or_else(|| "240".to_string());
    
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("hw.cpu.ncore=") {
            new_lines.push(format!("hw.cpu.ncore={}", final_cores));
            updated_cores = true;
        } else if trimmed.starts_with("hw.ramSize=") {
            new_lines.push(format!("hw.ramSize={}", final_ram));
            updated_ram = true;
        } else if trimmed.starts_with("hw.gpu.mode=") {
            new_lines.push(format!("hw.gpu.mode={}", gpu));
            updated_gpu = true;
        } else if trimmed.starts_with("vm.heapSize=") {
            new_lines.push(format!("vm.heapSize={}", heap_size));
            updated_heap = true;
        } else if trimmed.starts_with("hw.lcd.width=") {
            if !is_wear && resolution.is_some() {
                new_lines.push(format!("hw.lcd.width={}", w));
                updated_width = true;
            } else {
                new_lines.push(line.to_string());
            }
        } else if trimmed.starts_with("hw.lcd.height=") {
            if !is_wear && resolution.is_some() {
                new_lines.push(format!("hw.lcd.height={}", h));
                updated_height = true;
            } else {
                new_lines.push(line.to_string());
            }
        } else if trimmed.starts_with("hw.lcd.density=") {
            if !is_wear && dpi.is_some() {
                new_lines.push(format!("hw.lcd.density={}", density_val));
                updated_density = true;
            } else {
                new_lines.push(line.to_string());
            }
        } else if trimmed.starts_with("skin.name=") {
            if !is_wear && resolution.is_some() {
                new_lines.push(format!("skin.name={}", resolution.as_ref().unwrap()));
                updated_skin = true;
            } else {
                new_lines.push(line.to_string());
            }
        } else {
            new_lines.push(line.to_string());
        }
    }
    
    if !updated_cores { new_lines.push(format!("hw.cpu.ncore={}", final_cores)); }
    if !updated_ram { new_lines.push(format!("hw.ramSize={}", final_ram)); }
    if !updated_gpu { new_lines.push(format!("hw.gpu.mode={}", gpu)); }
    if !updated_heap { new_lines.push(format!("vm.heapSize={}", heap_size)); }
    if !is_wear && resolution.is_some() {
        if !updated_width { new_lines.push(format!("hw.lcd.width={}", w)); }
        if !updated_height { new_lines.push(format!("hw.lcd.height={}", h)); }
        if !updated_skin { new_lines.push(format!("skin.name={}", resolution.as_ref().unwrap())); }
    }
    if !is_wear && dpi.is_some() && !updated_density {
        new_lines.push(format!("hw.lcd.density={}", density_val));
    }
    
    match std::fs::write(&config_path, new_lines.join("\n")) {
        Ok(_) => CommandResult { ok: true, error: None, output: None },
        Err(e) => CommandResult { ok: false, error: Some(e.to_string()), output: None },
    }
}
