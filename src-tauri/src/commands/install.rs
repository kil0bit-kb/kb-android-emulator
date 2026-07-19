use std::path::Path;
use std::process::Stdio;
use tauri::{Emitter, Window};

#[cfg(windows)]
#[allow(unused_imports)]
use std::os::windows::process::CommandExt;

use super::paths::{avd_dir, cmdline_dir, jdk_dir, sdk_dir, build_env, get_java_exe};
use super::types::{CommandResult, InstallStatus};

// ─── Check Install Status ─────────────────────────────────────────────────────
#[tauri::command]
pub fn check_install_status() -> InstallStatus {
    let java_exe = get_java_exe();
    #[cfg(windows)]
    let emulator_installed = super::paths::emulator_dir().join("emulator.exe").exists();
    #[cfg(not(windows))]
    let emulator_installed = super::paths::emulator_dir().join("emulator").exists();

    #[cfg(windows)]
    let platform_tools_installed = sdk_dir().join("platform-tools").join("adb.exe").exists();
    #[cfg(not(windows))]
    let platform_tools_installed = sdk_dir().join("platform-tools").join("adb").exists();

    #[cfg(windows)]
    let cmdline_ok = cmdline_dir().join("bin").join("sdkmanager.bat").exists();
    #[cfg(not(windows))]
    let cmdline_ok = cmdline_dir().join("bin").join("sdkmanager").exists();

    let mut installed_packages = vec![];
    if emulator_installed {
        installed_packages.push("emulator".to_string());
    }
    if platform_tools_installed {
        installed_packages.push("platform-tools".to_string());
    }

    // Check system images dynamically by scanning the directories
    let sys_img_base = sdk_dir().join("system-images");
    if sys_img_base.exists() {
        if let Ok(api_entries) = std::fs::read_dir(&sys_img_base) {
            for api_entry in api_entries.filter_map(|e| e.ok()) {
                let api_name = api_entry.file_name().to_string_lossy().to_string();
                if let Ok(type_entries) = std::fs::read_dir(api_entry.path()) {
                    for type_entry in type_entries.filter_map(|e| e.ok()) {
                        let type_name = type_entry.file_name().to_string_lossy().to_string();
                        if let Ok(arch_entries) = std::fs::read_dir(type_entry.path()) {
                            for arch_entry in arch_entries.filter_map(|e| e.ok()) {
                                let arch_name = arch_entry.file_name().to_string_lossy().to_string();
                                if arch_entry.path().join("system.img").exists() {
                                    let pkg_id = format!("system-images;{};{};{}", api_name, type_name, arch_name);
                                    installed_packages.push(pkg_id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let licenses_accepted = {
        let licenses_dir = sdk_dir().join("licenses");
        licenses_dir.exists() && std::fs::read_dir(&licenses_dir).map(|mut d| d.next().is_some()).unwrap_or(false)
    };

    InstallStatus {
        jdk_installed: java_exe.map(|p| p.exists()).unwrap_or(false),
        cmdline_installed: cmdline_ok,
        emulator_installed,
        platform_tools_installed,
        avd_dir: avd_dir().to_string_lossy().to_string(),
        sdk_dir: sdk_dir().to_string_lossy().to_string(),
        installed_packages,
        licenses_accepted,
    }
}

// ─── Download helper (streaming with progress events) ─────────────────────────
pub async fn download_file_with_progress(
    url: &str,
    dest: &Path,
    task_key: &str,
    window: &Window,
) -> anyhow::Result<()> {
    use futures_util::StreamExt;

    // Reset cancellation flag before starting a fresh download
    super::set_download_cancelled(false);

    let client = reqwest::Client::builder()
        .user_agent("KB-Android-Manager/1.0")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    let resp = client.get(url).send().await?;
    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;

    let mut file = tokio::fs::File::create(dest).await?;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        // Check for cancellation on every chunk
        if super::is_download_cancelled() {
            drop(file);
            let _ = tokio::fs::remove_file(dest).await;
            anyhow::bail!("Download cancelled by user.");
        }
        let chunk = chunk?;
        downloaded += chunk.len() as u64;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
        if total > 0 {
            let pct = (downloaded * 100 / total) as u32;
            let _ = window.emit(
                "progress",
                serde_json::json!({ "task": task_key, "pct": pct, "dl": downloaded, "total": total }),
            );
        }
    }
    Ok(())
}

// ─── Cancel Download ────────────────────────────────────────────────────────────
#[tauri::command]
pub fn cancel_download() -> CommandResult {
    super::set_download_cancelled(true);
    CommandResult { ok: true, error: None, output: None }
}

pub fn extract_zip(zip_path: &Path, dest: &Path) -> anyhow::Result<()> {
    extract_zip_impl(zip_path, dest)
}

#[cfg(windows)]
fn extract_zip_impl(zip_path: &Path, dest: &Path) -> anyhow::Result<()> {
    use std::os::windows::process::CommandExt;
    let status = std::process::Command::new("powershell")
        .creation_flags(0x08000000)
        .args([
            "-Command",
            &format!(
                "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
                zip_path.display(),
                dest.display()
            ),
        ])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("PowerShell Expand-Archive failed")
    }
}

#[cfg(not(windows))]
fn extract_zip_impl(zip_path: &Path, dest: &Path) -> anyhow::Result<()> {
    let status = std::process::Command::new("unzip")
        .args([
            "-o",
            &zip_path.to_string_lossy(),
            "-d",
            &dest.to_string_lossy(),
        ])
        .status();
    match status {
        Ok(s) if s.success() => Ok(()),
        _ => {
            // Fallback to the zip crate if unzip is not available
            let file = std::fs::File::open(zip_path)?;
            let mut archive = zip::ZipArchive::new(file)?;
            archive.extract(dest)?;
            Ok(())
        }
    }
}

// ─── Install JDK ─────────────────────────────────────────────────────────────
#[tauri::command]
pub async fn install_jdk(window: Window) -> CommandResult {
    #[cfg(windows)]
    const JDK_URL: &str = "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.4%2B7/OpenJDK21U-jdk_x64_windows_hotspot_21.0.4_7.zip";
    #[cfg(target_os = "linux")]
    const JDK_URL: &str = "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.4%2B7/OpenJDK21U-jdk_x64_linux_hotspot_21.0.4_7.tar.gz";
    #[cfg(target_os = "macos")]
    const JDK_URL: &str = "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.4%2B7/OpenJDK21U-jdk_x64_mac_hotspot_21.0.4_7.tar.gz";

    #[cfg(windows)]
    let archive_name = "jdk.zip";
    #[cfg(not(windows))]
    let archive_name = "jdk.tar.gz";

    let archive_path = jdk_dir().join(archive_name);
    let _ = std::fs::create_dir_all(jdk_dir());

    let _ = window.emit("log", "Downloading portable OpenJDK 21 (Eclipse Temurin)...");

    if let Err(e) = download_file_with_progress(JDK_URL, &archive_path, "jdk", &window).await {
        return CommandResult { ok: false, error: Some(e.to_string()), output: None };
    }

    let _ = window.emit("log", "Extracting JDK...");

    #[cfg(windows)]
    {
        if let Err(e) = extract_zip(&archive_path, &jdk_dir()) {
            return CommandResult { ok: false, error: Some(e.to_string()), output: None };
        }
    }
    #[cfg(not(windows))]
    {
        let status = std::process::Command::new("tar")
            .args(["-xzf", &archive_path.to_string_lossy(), "-C", &jdk_dir().to_string_lossy()])
            .status();
        if !status.map(|s| s.success()).unwrap_or(false) {
            return CommandResult { ok: false, error: Some("tar extraction failed".to_string()), output: None };
        }
    }

    let _ = std::fs::remove_file(&archive_path);
    let _ = window.emit("log", "JDK installed successfully!");
    CommandResult { ok: true, error: None, output: None }
}

// ─── Install cmdline-tools ────────────────────────────────────────────────────
#[tauri::command]
pub async fn install_cmdline_tools(window: Window) -> CommandResult {
    #[cfg(windows)]
    const CMDLINE_URL: &str = "https://dl.google.com/android/repository/commandlinetools-win-11076708_latest.zip";
    #[cfg(target_os = "linux")]
    const CMDLINE_URL: &str = "https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip";
    #[cfg(target_os = "macos")]
    const CMDLINE_URL: &str = "https://dl.google.com/android/repository/commandlinetools-mac-11076708_latest.zip";

    let zip_path = sdk_dir().join("cmdline-tools.zip");
    let _ = std::fs::create_dir_all(sdk_dir());

    let _ = window.emit("log", "Downloading Android cmdline-tools from Google...");

    if let Err(e) = download_file_with_progress(CMDLINE_URL, &zip_path, "cmdline", &window).await {
        return CommandResult { ok: false, error: Some(e.to_string()), output: None };
    }

    let _ = window.emit("log", "Extracting cmdline-tools...");
    let temp_dir = sdk_dir().join("cmdline-tools-temp");
    let _ = std::fs::create_dir_all(&temp_dir);

    if let Err(e) = extract_zip(&zip_path, &temp_dir) {
        return CommandResult { ok: false, error: Some(e.to_string()), output: None };
    }
    let _ = std::fs::remove_file(&zip_path);

    // Move extracted/cmdline-tools -> sdk/cmdline-tools/latest
    let extracted = temp_dir.join("cmdline-tools");
    let dest_parent = sdk_dir().join("cmdline-tools");
    let _ = std::fs::create_dir_all(&dest_parent);
    let latest_dest = dest_parent.join("latest");
    if latest_dest.exists() {
        let _ = std::fs::remove_dir_all(&latest_dest);
    }
    if let Err(e) = std::fs::rename(&extracted, &latest_dest) {
        return CommandResult { ok: false, error: Some(e.to_string()), output: None };
    }
    let _ = std::fs::remove_dir_all(&temp_dir);

    // Make scripts executable on non-Windows platforms
    #[cfg(not(windows))]
    {
        let bin_dir = latest_dest.join("bin");
        if let Ok(entries) = std::fs::read_dir(&bin_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let _ = std::process::Command::new("chmod")
                    .args(["+x", &entry.path().to_string_lossy().to_string()])
                    .output();
            }
        }
    }

    let _ = window.emit("log", "cmdline-tools installed!");
    CommandResult { ok: true, error: None, output: None }
}

// A helper to extract percentage from lines like " [===================================] 35%"
pub fn extract_percentage(line: &str) -> Option<u32> {
    if let Some(pos) = line.find('%') {
        let mut start = pos;
        while start > 0 {
            let prev = start - 1;
            if let Some(c) = line.chars().nth(prev) {
                if c.is_ascii_digit() || c == ' ' {
                    start = prev;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        let number_part = line[start..pos].trim();
        number_part.parse::<u32>().ok()
    } else {
        None
    }
}

// ─── Run sdkmanager helper (Async & non-blocking) ──────────────────────────
pub async fn run_sdkmanager_async(args: &[&str], task_key: &str, window: &Window) -> anyhow::Result<String> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::io::AsyncWriteExt;

    #[cfg(windows)]
    let bat = cmdline_dir().join("bin").join("sdkmanager.bat");
    #[cfg(not(windows))]
    let bat = cmdline_dir().join("bin").join("sdkmanager");

    let env = build_env();
    let sdk_root_arg = format!("--sdk_root={}", sdk_dir().display());

    let mut all_args: Vec<String> = vec![];
    for arg in args {
        all_args.push(arg.to_string());
    }
    all_args.push(sdk_root_arg);

    let _ = window.emit("log", format!("Running sdkmanager {}", args.join(" ")));

    #[cfg(windows)]
    let (program, cmd_args) = {
        let args_str = all_args.iter().map(|a| format!("'{}'", a)).collect::<Vec<_>>().join(" ");
        let cmd_str = format!("& '{}' {}", bat.display(), args_str);
        ("powershell".to_string(), vec!["-NoProfile".to_string(), "-Command".to_string(), cmd_str])
    };
    #[cfg(not(windows))]
    let (program, mut cmd_args) = {
        let mut a = all_args.clone();
        a.insert(0, bat.to_string_lossy().to_string());
        ("sh".to_string(), a)
    };

    let mut cmd = tokio::process::Command::new(&program);
    cmd.args(&cmd_args)
       .envs(&env)
       .current_dir(cmdline_dir())
       .stdin(Stdio::piped())
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }

    let mut child = cmd.spawn()?;

    // Auto-accept licenses
    if let Some(mut stdin) = child.stdin.take() {
        tokio::spawn(async move {
            let _ = stdin.write_all(b"y\ny\ny\ny\ny\ny\ny\ny\ny\n").await;
        });
    }

    let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to open stdout"))?;
    let stderr = child.stderr.take().ok_or_else(|| anyhow::anyhow!("Failed to open stderr"))?;

    let window_clone = window.clone();
    let task_key_clone = task_key.to_string();

    // Reader task for stdout
    let stdout_handle = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut reader = BufReader::new(stdout);
        let mut buf = [0u8; 1024];
        let mut line_accumulator = Vec::new();
        let mut full_output = String::new();
        
        while let Ok(n) = reader.read(&mut buf).await {
            if n == 0 { break; }
            for &byte in &buf[..n] {
                if byte == b'\n' || byte == b'\r' {
                    if !line_accumulator.is_empty() {
                        if let Ok(line) = std::str::from_utf8(&line_accumulator) {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                let _ = window_clone.emit("log", trimmed.to_string());
                                full_output.push_str(trimmed);
                                full_output.push('\n');
                                
                                if let Some(pct) = extract_percentage(trimmed) {
                                    let _ = window_clone.emit(
                                        "progress",
                                        serde_json::json!({ "task": task_key_clone, "pct": pct }),
                                    );
                                }
                            }
                        }
                        line_accumulator.clear();
                    }
                } else {
                    line_accumulator.push(byte);
                }
            }
        }
        full_output
    });

    let window_clone_err = window.clone();
    // Reader task for stderr
    let stderr_handle = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if !line.trim().is_empty() {
                let _ = window_clone_err.emit("log", format!("[warn] {}", line));
            }
        }
    });

    let status = child.wait().await?;
    let full_stdout = stdout_handle.await.unwrap_or_default();
    let _ = stderr_handle.await;

    if status.success() {
        Ok(full_stdout)
    } else {
        anyhow::bail!("sdkmanager exited with code {:?}", status.code())
    }
}

// ─── Accept Licenses ──────────────────────────────────────────────────────────
#[tauri::command]
pub fn accept_licenses(window: Window) -> CommandResult {
    #[cfg(windows)]
    let bat = cmdline_dir().join("bin").join("sdkmanager.bat");
    #[cfg(not(windows))]
    let bat = cmdline_dir().join("bin").join("sdkmanager");

    let env = build_env();
    let sdk_root_arg = format!("--sdk_root={}", sdk_dir().display());

    #[cfg(windows)]
    let cmd_str = format!("& '{}' --licenses '{}'", bat.display(), sdk_root_arg);

    #[cfg(windows)]
    let mut child = {
        use std::os::windows::process::CommandExt;
        match std::process::Command::new("powershell")
            .creation_flags(0x08000000)
            .args(["-NoProfile", "-Command", &cmd_str])
            .envs(&env)
            .current_dir(cmdline_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return CommandResult { ok: false, error: Some(e.to_string()), output: None },
        }
    };

    #[cfg(not(windows))]
    let mut child = match std::process::Command::new(&bat)
        .args(["--licenses", &sdk_root_arg])
        .envs(&env)
        .current_dir(cmdline_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return CommandResult { ok: false, error: Some(e.to_string()), output: None },
    };

    if let Some(stdin) = child.stdin.take() {
        let mut s = stdin;
        use std::io::Write;
        let _ = s.write_all(b"y\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\n");
    }

    let out = child.wait_with_output().unwrap();
    let _ = window.emit("log", "SDK licenses accepted.");
    CommandResult { ok: out.status.success(), error: None, output: None }
}

// ─── Install SDK package ──────────────────────────────────────────────────────
#[tauri::command]
pub async fn install_package(package_id: String, window: Window) -> CommandResult {
    match run_sdkmanager_async(&["--install", &package_id], &package_id, &window).await {
        Ok(out) => {
            let _ = window.emit("log", format!("Installed: {}", package_id));
            let _ = window.emit(
                "progress",
                serde_json::json!({ "task": package_id, "pct": 100 }),
            );
            CommandResult { ok: true, error: None, output: Some(out) }
        }
        Err(e) => {
            let _ = window.emit("log", format!("Failed: {}", e));
            CommandResult { ok: false, error: Some(e.to_string()), output: None }
        }
    }
}

// ─── Uninstall SDK package ────────────────────────────────────────────────────
#[tauri::command]
pub async fn uninstall_package(package_id: String, window: Window) -> CommandResult {
    match run_sdkmanager_async(&["--uninstall", &package_id], &package_id, &window).await {
        Ok(out) => {
            let _ = window.emit("log", format!("Uninstalled: {}", package_id));
            let _ = window.emit(
                "progress",
                serde_json::json!({ "task": package_id, "pct": 100 }),
            );
            CommandResult { ok: true, error: None, output: Some(out) }
        }
        Err(e) => {
            let _ = window.emit("log", format!("Failed to uninstall: {}", e));
            CommandResult { ok: false, error: Some(e.to_string()), output: None }
        }
    }
}

// ─── Fetch SDK Packages ────────────────────────────────────────────────────────
#[tauri::command]
pub async fn fetch_sdk_packages(window: Window) -> Result<Vec<super::types::SdkPackageInfo>, String> {
    #[cfg(windows)]
    let bat = cmdline_dir().join("bin").join("sdkmanager.bat");
    #[cfg(not(windows))]
    let bat = cmdline_dir().join("bin").join("sdkmanager");

    let env = build_env();

    #[cfg(windows)]
    let cmd_str = format!("& '{}' --list", bat.display());

    #[cfg(windows)]
    let child = {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("powershell")
            .creation_flags(0x08000000)
            .args(["-NoProfile", "-Command", &cmd_str])
            .envs(&env)
            .current_dir(cmdline_dir())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?
    };

    #[cfg(not(windows))]
    let child = std::process::Command::new(&bat)
        .args(["--list"])
        .envs(&env)
        .current_dir(cmdline_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;

    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();

    let mut candidates = vec![];
    let mut is_installed_section = true;

    struct Candidate {
        id: String,
        api_level: i32,
        version: String,
        desc: String,
        is_16k: bool,
        is_installed: bool,
        variant: String,
    }

    for line in stdout.lines() {
        let line_trimmed = line.trim();
        if line_trimmed.starts_with("Installed packages:") {
            is_installed_section = true;
            continue;
        } else if line_trimmed.starts_with("Available Packages:") {
            is_installed_section = false;
            continue;
        }

        if line_trimmed.starts_with("system-images;") {
            let parts: Vec<&str> = line_trimmed.split('|').collect();
            if parts.len() >= 2 {
                let id = parts[0].trim().to_string();

                // FILTER: Only keep x86_64 system images for native PC speed
                if !id.contains("x86_64") {
                    continue;
                }

                // FILTER: Exclude SDK Extensions
                if id.contains("-ext") || id.contains("_ext") {
                    continue;
                }

                // FILTER: Exclude raw codenames
                let id_lower = id.to_lowercase();
                if id_lower.contains("canary") || id_lower.contains("cinnamonbun") {
                    continue;
                }

                let id_split: Vec<&str> = id.split(';').collect();
                if id_split.len() >= 4 {
                    let api_level_str = id_split[1]
                        .replace("android-", "")
                        .split('.')
                        .next()
                        .unwrap_or("")
                        .to_string();

                    // Only keep modern versions (API 30+ / Android 11+)
                    if let Ok(api_level_num) = api_level_str.parse::<i32>() {
                        if api_level_num < 30 {
                            continue;
                        }

                        let is_16k = id.contains("ps16k") || id.contains("16k") || id.contains("16kb");
                        let version = parts[1].trim().to_string();
                        let desc = if parts.len() >= 3 {
                            parts[2].trim().to_string()
                        } else {
                            "".to_string()
                        };

                        let variant = id_split[2].to_string();

                        candidates.push(Candidate {
                            id,
                            api_level: api_level_num,
                            version,
                            desc,
                            is_16k,
                            is_installed: is_installed_section,
                            variant,
                        });
                    }
                }
            }
        }
    }

    // Find the latest API level in the fetched list dynamically
    let max_api_level = candidates.iter().map(|c| c.api_level).max().unwrap_or(36);

    let mut packages = vec![];
    for c in candidates {
        if c.is_16k && c.api_level != max_api_level {
            continue;
        }

        let id_lower = c.id.to_lowercase();
        let is_beta = c.is_16k || 
                      id_lower.contains("preview") || 
                      id_lower.contains("beta");

        let category = if c.id.contains("android-tv") || c.id.contains("google-tv") {
            "Android TV".to_string()
        } else if c.id.contains("android-wear") || c.id.contains("wear") {
            "Wear OS".to_string()
        } else if c.id.contains("android-automotive") {
            "Automotive".to_string()
        } else if is_beta {
            "Beta / Preview".to_string()
        } else if c.id.contains("playstore") {
            "Stable (Play Store)".to_string()
        } else {
            "Stable (Google APIs)".to_string()
        };

        let android_ver = if c.api_level >= 33 {
            format!("{}", c.api_level - 20)
        } else if c.api_level == 31 || c.api_level == 32 {
            "12".to_string()
        } else if c.api_level == 30 {
            "11".to_string()
        } else {
            c.api_level.to_string()
        };

        let ver_suffix = if is_beta { " (Beta/Preview)" } else { "" };
        let page_size_suffix = if c.is_16k { " [16 KB Page Size]" } else { "" };

        let name = format!(
            "Android {}{} (API {}) · {}{}",
            android_ver,
            ver_suffix,
            c.api_level,
            c.variant.replace("_", " ").to_uppercase(),
            page_size_suffix
        );

        packages.push(super::types::SdkPackageInfo {
            id: c.id,
            name,
            desc: c.desc,
            version: c.version,
            installed: c.is_installed,
            category,
        });
    }

    // Deduplicate and prioritize installed status
    let mut unique_packages: std::collections::HashMap<String, super::types::SdkPackageInfo> = std::collections::HashMap::new();
    for p in packages {
        if let Some(existing) = unique_packages.get_mut(&p.id) {
            if p.installed {
                existing.installed = true;
            }
        } else {
            unique_packages.insert(p.id.clone(), p);
        }
    }

    let mut result: Vec<super::types::SdkPackageInfo> = unique_packages.into_values().collect();
    result.sort_by(|a, b| b.id.cmp(&a.id));
    
    let _ = window.emit("log", format!("Fetched {} system images from Google repositories.", result.len()));
    Ok(result)
}

// ─── Uninstall JDK ───────────────────────────────────────────────────────────
#[tauri::command]
pub fn uninstall_jdk(window: Window) -> CommandResult {
    let dir = jdk_dir();
    let _ = window.emit("log", "Uninstalling JDK...");
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            return CommandResult { ok: false, error: Some(format!("Failed to delete JDK directory: {}", e)), output: None };
        }
    }
    // Re-create the empty directory so structure is maintained
    let _ = std::fs::create_dir_all(&dir);
    let _ = window.emit("log", "JDK uninstalled successfully!");
    CommandResult { ok: true, error: None, output: None }
}

// ─── Uninstall cmdline-tools ──────────────────────────────────────────────────
#[tauri::command]
pub fn uninstall_cmdline_tools(window: Window) -> CommandResult {
    let dir = sdk_dir().join("cmdline-tools");
    let _ = window.emit("log", "Uninstalling cmdline-tools...");
    if dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&dir) {
            return CommandResult { ok: false, error: Some(format!("Failed to delete cmdline-tools directory: {}", e)), output: None };
        }
    }
    let _ = window.emit("log", "cmdline-tools uninstalled successfully!");
    CommandResult { ok: true, error: None, output: None }
}

