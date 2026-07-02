use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use tauri::{Emitter, Window};

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> *mut std::ffi::c_void;
    fn SetPriorityClass(hProcess: *mut std::ffi::c_void, dwPriorityClass: u32) -> i32;
    fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
}

const PROCESS_SET_INFORMATION: u32 = 0x0200;
const HIGH_PRIORITY_CLASS: u32 = 0x00000080;

// ─── Global running emulators map ────────────────────────────────────────────
pub struct RunningEmulators(pub Mutex<HashMap<String, u32>>); // name -> pid

use std::sync::OnceLock;
static RUNNING_AVDS: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();

fn get_running_avds() -> &'static Mutex<HashMap<String, bool>> {
    RUNNING_AVDS.get_or_init(|| Mutex::new(HashMap::new()))
}

// ─── Path helpers ─────────────────────────────────────────────────────────────
fn sdk_base() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    let dir = exe.parent().unwrap_or(Path::new("."));

    // Traverse upwards to check if we are in development mode (project contains src-tauri folder)
    let mut current = dir.to_path_buf();
    for _ in 0..10 {
        if current.join("src-tauri").exists() {
            return current.join("android-sdk");
        }
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            break;
        }
    }

    // Production fallback: next to the executable
    dir.join("android-sdk")
}

fn jdk_dir() -> PathBuf {
    sdk_base().join("jdk")
}
fn sdk_dir() -> PathBuf {
    sdk_base().join("sdk")
}
fn cmdline_dir() -> PathBuf {
    sdk_dir().join("cmdline-tools").join("latest")
}
fn emulator_dir() -> PathBuf {
    sdk_dir().join("emulator")
}
fn avd_dir() -> PathBuf {
    sdk_base().join("avd")
}

pub fn ensure_dirs(base: &PathBuf) {
    let _ = base; // we use sdk_base() internally
    for d in [sdk_base(), jdk_dir(), sdk_dir(), avd_dir()] {
        let _ = std::fs::create_dir_all(&d);
    }
}

fn get_java_exe() -> Option<PathBuf> {
    let jdk = jdk_dir();
    if !jdk.exists() {
        return None;
    }
    let entry = std::fs::read_dir(&jdk)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("jdk")
        })?;
    let java = entry.path().join("bin").join("java.exe");
    if java.exists() {
        Some(java)
    } else {
        None
    }
}

fn get_java_home() -> Option<PathBuf> {
    let jdk = jdk_dir();
    let entry = std::fs::read_dir(&jdk)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().starts_with("jdk"))?;
    Some(entry.path())
}

fn build_env() -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();
    if let Some(jh) = get_java_home() {
        env.insert("JAVA_HOME".to_string(), jh.to_string_lossy().to_string());
        let bin = jh.join("bin").to_string_lossy().to_string();
        let path = env.get("PATH").cloned().unwrap_or_default();
        env.insert("PATH".to_string(), format!("{};{}", bin, path));
    }
    env.insert("ANDROID_SDK_ROOT".to_string(), sdk_dir().to_string_lossy().to_string());
    env.insert("ANDROID_AVD_HOME".to_string(), avd_dir().to_string_lossy().to_string());
    env.insert("ANDROID_EMULATOR_HOME".to_string(), avd_dir().to_string_lossy().to_string());
    
    // Low-latency WASAPI Audio Optimizations
    env.insert("QEMU_AUDIO_TIMER_PERIOD".to_string(), "0".to_string());
    env.insert("QEMU_WASAPI_BUF_SIZE".to_string(), "512".to_string());
    
    // Force ANGLE (Direct3D11/DirectX translation) on Windows
    env.insert("ANDROID_EMULATOR_USE_ANGLE".to_string(), "1".to_string());
    
    env
}

// ─── Structs ──────────────────────────────────────────────────────────────────
#[derive(Serialize, Deserialize, Clone)]
pub struct SystemInfo {
    pub total_ram: u64,
    pub free_ram: u64,
    pub cpu_count: usize,
    pub cpu_model: String,
    pub platform: String,
    pub arch: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GpuInfo {
    pub index: usize,
    pub name: String,
    pub vram: String,
    pub is_dedicated: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct InstallStatus {
    pub jdk_installed: bool,
    pub cmdline_installed: bool,
    pub emulator_installed: bool,
    pub platform_tools_installed: bool,
    pub avd_dir: String,
    pub sdk_dir: String,
    pub installed_packages: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AvdInfo {
    pub name: String,
    pub path: String,
    pub target: String,
    pub api: String,
    pub ram: String,
    pub cores: String,
    pub gpu: String,
    pub running: bool,
}

#[derive(Serialize, Deserialize)]
pub struct CreateAvdOptions {
    pub name: String,
    pub system_image: String,
    pub ram: u32,
    pub cores: u32,
    pub storage: u32,
    pub gpu_mode: String,
    pub screen_resolution: String,
    pub dpi: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CommandResult {
    pub ok: bool,
    pub error: Option<String>,
    pub output: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct HypervisorStatus {
    pub whpx_enabled: bool,
    pub vtx_enabled: bool,
}

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

// ─── System Info ──────────────────────────────────────────────────────────────
#[tauri::command]
pub fn get_system_info() -> SystemInfo {
    let cpu_count = num_cpus();
    SystemInfo {
        total_ram: total_memory_mb(),
        free_ram: free_memory_mb(),
        cpu_count,
        cpu_model: cpu_model(),
        platform: "windows".to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

fn total_memory_mb() -> u64 {
    // Query via WMI/GlobalMemoryStatusEx via cmd
    let out = std::process::Command::new("powershell")
        .args(["-Command", "(Get-WmiObject Win32_ComputerSystem).TotalPhysicalMemory"])
        .output();
    if let Ok(o) = out {
        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if let Ok(bytes) = s.parse::<u64>() {
            return bytes / (1024 * 1024);
        }
    }
    8192
}

fn free_memory_mb() -> u64 {
    let out = std::process::Command::new("powershell")
        .args(["-Command", "(Get-WmiObject Win32_OperatingSystem).FreePhysicalMemory"])
        .output();
    if let Ok(o) = out {
        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if let Ok(kb) = s.parse::<u64>() {
            return kb / 1024;
        }
    }
    4096
}

fn cpu_model() -> String {
    let out = std::process::Command::new("powershell")
        .args(["-Command", "(Get-WmiObject Win32_Processor).Name"])
        .output();
    if let Ok(o) = out {
        return String::from_utf8_lossy(&o.stdout).trim().to_string();
    }
    "Unknown CPU".to_string()
}

// ─── GPU Detection ────────────────────────────────────────────────────────────
#[tauri::command]
pub fn detect_gpus() -> Vec<GpuInfo> {
    let out = std::process::Command::new("powershell")
        .args([
            "-Command",
            "Get-WmiObject Win32_VideoController | Select-Object Name, AdapterRAM | ConvertTo-Json",
        ])
        .output();

    let Ok(o) = out else { return vec![] };
    let text = String::from_utf8_lossy(&o.stdout).to_string();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);

    let arr = match json {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(_) => vec![json],
        _ => return vec![],
    };

    arr.into_iter()
        .enumerate()
        .map(|(i, g)| {
            let name = g["Name"].as_str().unwrap_or("Unknown GPU").to_string();
            let ram_bytes = g["AdapterRAM"].as_u64().unwrap_or(0);
            let vram = if ram_bytes > 0 {
                format!("{} MB", ram_bytes / (1024 * 1024))
            } else {
                "Unknown".to_string()
            };
            let is_dedicated = name.to_lowercase().contains("nvidia")
                || name.to_lowercase().contains("amd")
                || name.to_lowercase().contains("radeon")
                || name.to_lowercase().contains("geforce")
                || name.to_lowercase().contains("rtx")
                || name.to_lowercase().contains("gtx");
            GpuInfo { index: i, name, vram, is_dedicated }
        })
        .collect()
}

// ─── Set GPU Preference (Windows Registry) ────────────────────────────────────
#[tauri::command]
pub fn set_gpu_preference(_gpu_index: usize, gpu_preference: u32) -> CommandResult {
    use winreg::enums::*;
    use winreg::RegKey;

    let emulator_exe = emulator_dir().join("emulator.exe");
    let qemu_exe = emulator_dir()
        .join("qemu")
        .join("windows-x86_64")
        .join("qemu-system-x86_64.exe");

    let targets: Vec<PathBuf> = vec![emulator_exe, qemu_exe]
        .into_iter()
        .filter(|p| p.exists())
        .collect();

    if targets.is_empty() {
        return CommandResult {
            ok: false,
            error: Some("Emulator not installed yet. Install the emulator first.".to_string()),
            output: None,
        };
    }

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(r"Software\Microsoft\DirectX\UserGpuPreferences")
        .unwrap();

    let mut errors = vec![];
    for target in &targets {
        let value_data = format!("GpuPreference={};", gpu_preference);
        if let Err(e) = key.set_value(target.to_string_lossy().as_ref(), &value_data) {
            errors.push(e.to_string());
        }
    }

    if errors.is_empty() {
        CommandResult {
            ok: true,
            error: None,
            output: Some(format!(
                "GPU preference {} applied to {} executables",
                gpu_preference,
                targets.len()
            )),
        }
    } else {
        CommandResult {
            ok: false,
            error: Some(errors.join("; ")),
            output: None,
        }
    }
}

// ─── Check Hypervisor ─────────────────────────────────────────────────────────
#[tauri::command]
pub fn check_hypervisor() -> HypervisorStatus {
    // Check if Hypervisor is active (WSL2 / Hyper-V / WHPX is running).
    // This query is instant and does NOT require Administrator elevation.
    let hypervisor_present_out = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance -ClassName Win32_ComputerSystem).HypervisorPresent",
        ])
        .output();

    let hypervisor_present = hypervisor_present_out
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_lowercase() == "true")
        .unwrap_or(false);

    if hypervisor_present {
        // If the Hypervisor is active, then VT-x/AMD-V firmware virtualizations are enabled
        // and WHPX is actively available.
        return HypervisorStatus {
            whpx_enabled: true,
            vtx_enabled: true,
        };
    }

    // Fallback: If hypervisor is NOT present, query if VT-x/AMD-V is enabled in BIOS
    let vtx = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance -ClassName Win32_Processor).VirtualizationFirmwareEnabled",
        ])
        .output();
    let vtx_enabled = vtx
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_lowercase() == "true")
        .unwrap_or(false);

    HypervisorStatus { whpx_enabled: false, vtx_enabled }
}

// ─── Enable WHPX ─────────────────────────────────────────────────────────────
#[tauri::command]
pub fn enable_whpx() -> CommandResult {
    let out = std::process::Command::new("powershell")
        .args([
            "-Command",
            "Enable-WindowsOptionalFeature -Online -FeatureName HypervisorPlatform -NoRestart",
        ])
        .output();
    match out {
        Ok(o) => CommandResult {
            ok: o.status.success(),
            error: if !o.status.success() {
                Some(String::from_utf8_lossy(&o.stderr).to_string())
            } else {
                None
            },
            output: Some(String::from_utf8_lossy(&o.stdout).to_string()),
        },
        Err(e) => CommandResult {
            ok: false,
            error: Some(e.to_string()),
            output: None,
        },
    }
}

// ─── Check Install Status ─────────────────────────────────────────────────────
#[tauri::command]
pub fn check_install_status() -> InstallStatus {
    let java_exe = get_java_exe();
    let emulator_installed = emulator_dir().join("emulator.exe").exists();
    let platform_tools_installed = sdk_dir().join("platform-tools").join("adb.exe").exists();

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
                                // If system.img is present, this system image is fully installed
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

    InstallStatus {
        jdk_installed: java_exe.map(|p| p.exists()).unwrap_or(false),
        cmdline_installed: cmdline_dir().join("bin").join("sdkmanager.bat").exists(),
        emulator_installed,
        platform_tools_installed,
        avd_dir: avd_dir().to_string_lossy().to_string(),
        sdk_dir: sdk_dir().to_string_lossy().to_string(),
        installed_packages,
    }
}

// ─── Download helper (streaming with progress events) ─────────────────────────
async fn download_file_with_progress(
    url: &str,
    dest: &Path,
    task_key: &str,
    window: &Window,
) -> anyhow::Result<()> {
    use futures_util::StreamExt;

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

fn extract_zip(zip_path: &Path, dest: &Path) -> anyhow::Result<()> {
    let status = std::process::Command::new("powershell")
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

// ─── Install JDK ─────────────────────────────────────────────────────────────
#[tauri::command]
pub async fn install_jdk(window: Window) -> CommandResult {
    const JDK_URL: &str = "https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.4%2B7/OpenJDK21U-jdk_x64_windows_hotspot_21.0.4_7.zip";
    let zip_path = jdk_dir().join("jdk.zip");
    let _ = std::fs::create_dir_all(jdk_dir());

    let _ = window.emit("log", "📥 Downloading portable OpenJDK 21 (Eclipse Temurin)...");

    if let Err(e) = download_file_with_progress(JDK_URL, &zip_path, "jdk", &window).await {
        return CommandResult { ok: false, error: Some(e.to_string()), output: None };
    }

    let _ = window.emit("log", "📦 Extracting JDK...");
    if let Err(e) = extract_zip(&zip_path, &jdk_dir()) {
        return CommandResult { ok: false, error: Some(e.to_string()), output: None };
    }

    let _ = std::fs::remove_file(&zip_path);
    let _ = window.emit("log", "✅ JDK installed successfully!");
    CommandResult { ok: true, error: None, output: None }
}

// ─── Install cmdline-tools ────────────────────────────────────────────────────
#[tauri::command]
pub async fn install_cmdline_tools(window: Window) -> CommandResult {
    const CMDLINE_URL: &str = "https://dl.google.com/android/repository/commandlinetools-win-11076708_latest.zip";
    let zip_path = sdk_dir().join("cmdline-tools.zip");
    let _ = std::fs::create_dir_all(sdk_dir());

    let _ = window.emit("log", "📥 Downloading Android cmdline-tools from Google...");

    if let Err(e) = download_file_with_progress(CMDLINE_URL, &zip_path, "cmdline", &window).await {
        return CommandResult { ok: false, error: Some(e.to_string()), output: None };
    }

    let _ = window.emit("log", "📦 Extracting cmdline-tools...");
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

    let _ = window.emit("log", "✅ cmdline-tools installed!");
    CommandResult { ok: true, error: None, output: None }
}

// A helper to extract percentage from lines like " [===================================] 35%"
fn extract_percentage(line: &str) -> Option<u32> {
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
async fn run_sdkmanager_async(args: &[&str], task_key: &str, window: &Window) -> anyhow::Result<String> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::io::AsyncWriteExt;

    let bat = cmdline_dir().join("bin").join("sdkmanager.bat");
    let env = build_env();
    let sdk_root_arg = format!("--sdk_root={}", sdk_dir().display());

    let mut all_args: Vec<String> = vec![];
    for arg in args {
        all_args.push(arg.to_string());
    }
    all_args.push(sdk_root_arg);

    let _ = window.emit("log", format!("▶ sdkmanager {}", args.join(" ")));

    // Use PowerShell with call operator and single quotes to handle paths with spaces safely
    let args_str = all_args.iter().map(|a| format!("'{}'", a)).collect::<Vec<_>>().join(" ");
    let cmd_str = format!("& '{}' {}", bat.display(), args_str);

    let mut cmd = tokio::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &cmd_str])
       .envs(&env)
       .current_dir(cmdline_dir())
       .stdin(Stdio::piped())
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());

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
    let bat = cmdline_dir().join("bin").join("sdkmanager.bat");
    let env = build_env();
    let sdk_root_arg = format!("--sdk_root={}", sdk_dir().display());

    let cmd_str = format!("& '{}' --licenses '{}'", bat.display(), sdk_root_arg);

    let mut child = match std::process::Command::new("powershell")
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
    };

    if let Some(stdin) = child.stdin.take() {
        let mut s = stdin;
        use std::io::Write;
        let _ = s.write_all(b"y\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\ny\n");
    }

    let out = child.wait_with_output().unwrap();
    let _ = window.emit("log", "✅ SDK licenses accepted.");
    CommandResult { ok: out.status.success(), error: None, output: None }
}

// ─── Install SDK package ──────────────────────────────────────────────────────
#[tauri::command]
pub async fn install_package(package_id: String, window: Window) -> CommandResult {
    match run_sdkmanager_async(&["--install", &package_id], &package_id, &window).await {
        Ok(out) => {
            let _ = window.emit("log", format!("✅ Installed: {}", package_id));
            let _ = window.emit(
                "progress",
                serde_json::json!({ "task": package_id, "pct": 100 }),
            );
            CommandResult { ok: true, error: None, output: Some(out) }
        }
        Err(e) => {
            let _ = window.emit("log", format!("❌ Failed: {}", e));
            CommandResult { ok: false, error: Some(e.to_string()), output: None }
        }
    }
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
    let bat = cmdline_dir().join("bin").join("avdmanager.bat");
    let env = build_env();
    let _sdk_root_arg = format!("--sdk_root={}", sdk_dir().display());

    // Sanitize the AVD name (replace spaces with underscores) as Android SDK requires this
    let sanitized_name = options.name.replace(" ", "_");

    let _ = window.emit("log", format!("▶ Creating AVD: {} (sanitized as: {})", options.name, sanitized_name));

    // Pipe "no" directly to answer the custom hardware profile question inside PowerShell shell environment
    let cmd_str = format!(
        "echo no | & '{}' create avd --name '{}' --package '{}' --force",
        bat.display(),
        sanitized_name,
        options.system_image
    );

    let mut cmd = tokio::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &cmd_str])
       .envs(&env)
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

    if config_path.exists() {
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

    let _ = window.emit("log", format!("✅ AVD \"{}\" created successfully with high-performance config!", sanitized_name));
    CommandResult { ok: true, error: None, output: None }
}

fn build_perf_config(opts: &CreateAvdOptions) -> Vec<(String, String)> {
    let (w, h) = opts.screen_resolution.split_once('x').unwrap_or(("1280", "720"));
    
    // Dynamic Heap calculation based on RAM size:
    // If >= 8GB (8192MB), max heap is 1024MB.
    // If < 8GB, max heap is 512MB.
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
    let bat = cmdline_dir().join("bin").join("avdmanager.bat");
    let env = build_env();

    // Use PowerShell call operator with single quotes for space safety
    let cmd_str = format!("& '{}' delete avd --name '{}'", bat.display(), name);

    let mut cmd = tokio::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-Command", &cmd_str])
       .envs(&env)
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

    let _ = window.emit("log", format!("🗑️ Deleted AVD: {}", name));
    CommandResult { ok: out.status.success(), error: None, output: None }
}

fn auto_tune_heap_before_launch(name: &str) -> u64 {
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
    ram_val
}

fn auto_repair_wear_os_config(name: &str) {
    let config_path = avd_dir().join(format!("{}.avd", name)).join("config.ini");
    if !config_path.exists() { return; }
    
    if let Ok(content) = std::fs::read_to_string(&config_path) {
        let mut is_wear = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("tag.id=") {
                let id = trimmed.split('=').nth(1).unwrap_or("").trim();
                if id.contains("wear") { is_wear = true; }
            }
            if trimmed.starts_with("image.sysdir.1=") {
                let sysdir = trimmed.split('=').nth(1).unwrap_or("").trim();
                if sysdir.contains("wear") { is_wear = true; }
            }
        }
        
        if is_wear {
            let mut clean_lines = vec![];
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("hw.lcd.width=")
                    || trimmed.starts_with("hw.lcd.height=")
                    || trimmed.starts_with("hw.lcd.density=")
                    || trimmed.starts_with("skin.name=")
                    || trimmed.starts_with("skin.path=")
                {
                    continue;
                }
                if trimmed.starts_with("hw.cpu.ncore=") {
                    clean_lines.push("hw.cpu.ncore=1".to_string());
                } else if trimmed.starts_with("hw.ramSize=") {
                    clean_lines.push("hw.ramSize=1024".to_string());
                } else {
                    clean_lines.push(line.to_string());
                }
            }
            let _ = std::fs::write(&config_path, clean_lines.join("\n"));
        }
    }
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
    window: Window,
) -> CommandResult {
    // Auto-repair Wear OS configuration if it has corrupted phone settings
    auto_repair_wear_os_config(&name);

    let emulator_exe = emulator_dir().join("emulator.exe");
    if !emulator_exe.exists() {
        return CommandResult {
            ok: false,
            error: Some("Emulator not installed. Install it from SDK Manager first.".to_string()),
            output: None,
        };
    }

    // Auto-tune heap size on launch and get RAM
    let ram_size = auto_tune_heap_before_launch(&name);

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
    // Valid values for -accel are now: on, off, auto
    if accelerator == "whpx" || accelerator == "aehd" || accelerator == "haxm" {
        accelerator = "on".to_string();
    }

    let avd_home = avd_dir().to_string_lossy().to_string();

    let _ = window.emit(
        "log",
        format!("🚀 Launching \"{}\" — GPU={}, accel={}", name, gpu, accelerator),
    );

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

    let mut args = vec![
        "-avd".to_string(),
        name.clone(),
        "-gpu".to_string(),
        gpu,
        "-accel".to_string(),
        accelerator,
        "-feature".to_string(),
        "Vulkan,GLESDynamicVersion".to_string(),
        // Always silence verbose guest logcat — huge disk I/O reduction
        "-logcat".to_string(),
        "*:S".to_string(),
        // Opt out of telemetry/metrics — suppresses warning banner + background reporter
        "-no-metrics".to_string(),
        // Force guest Android OS overrides to apply on boot using -prop command line switches (prefixed with qemu.)
        "-prop".to_string(),
        format!("qemu.dalvik.vm.heapsize={}", heap_sz),
        "-prop".to_string(),
        format!("qemu.dalvik.vm.heapgrowthlimit={}", growth_lim),
        // Optimize TCP socket buffer sizes for network downloads (prefixed with qemu.)
        "-prop".to_string(),
        "qemu.net.tcp.buffersize.default=4096,87380,110208,4096,16384,110208".to_string(),
        "-prop".to_string(),
        "qemu.net.tcp.buffersize.wifi=262144,524288,1048576,262144,524288,1048576".to_string(),
        // Use fast Cloudflare DNS to bypass slirp DNS routing latency
        "-dns-server".to_string(),
        "1.1.1.1".to_string(),
        // Use WASAPI for audio output/input on Windows
        "-audio".to_string(),
        "wasapi".to_string(),
    ];

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

    // netsim-args: only --no-cli-ui is confirmed supported; bluetooth is optional
    let mut netsim_flags: Vec<&str> = vec!["--no-cli-ui"];
    if let Some(true) = no_bluetooth {
        netsim_flags.push("--no-bluetooth");
    }
    for flag in &netsim_flags {
        args.push("-netsim-args".to_string());
        args.push(flag.to_string());
    }

    let emulator_cwd = emulator_dir();
    // Spawn the emulator detached and pipe stdout; stderr is null-routed because
    // the emulator mirrors every log line to both streams — reading one is enough.
    let child = std::process::Command::new(&emulator_exe)
        .current_dir(&emulator_cwd)
        .args(&args)
        .env("ANDROID_AVD_HOME", &avd_home)
        .env("ANDROID_EMULATOR_HOME", &avd_home)
        .envs(&env)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())   // ← drop stderr, it's a mirror of stdout
        .spawn();

    match child {
        Ok(mut c) => {
            let pid = c.id();

            // Elevate process priority to HIGH_PRIORITY_CLASS (0x00000080)
            unsafe {
                let handle = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
                if !handle.is_null() {
                    let success = SetPriorityClass(handle, HIGH_PRIORITY_CLASS);
                    println!("[Backend] Elevated emulator.exe (PID {}) priority class to High: {}", pid, success != 0);
                    CloseHandle(handle);
                }
            }

            get_running_avds().lock().unwrap().insert(name.clone(), true);
            let name_clone = name.clone();
            let window_clone = window.clone();

            let stdout = c.stdout.take();

            // Read only stdout (stderr is null-routed above).
            // The emulator dumps the ENTIRE startup log to stdout first, then the
            // ENTIRE log again to stderr — both merged into the same pipe on Windows.
            // Duplicates arrive ~150+ lines apart so we need a window of 256 to
            // cover the full startup batch. After boot, the window naturally ages out
            // old startup lines and runtime lines flow through unaffected.
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
                            if seen.contains(&trimmed) { continue; }  // duplicate — drop it
                            seen.push_back(trimmed.clone());
                            if seen.len() > 256 { seen.pop_front(); }  // keep window bounded
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
            // Monitor the process in a thread
            std::thread::spawn(move || {
                let _ = c.wait();
                get_running_avds().lock().unwrap().remove(&name_clone2);
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
    // Use ADB to gracefully shut down
    let adb_exe = sdk_dir().join("platform-tools").join("adb.exe");
    if adb_exe.exists() {
        let _ = std::process::Command::new(&adb_exe)
            .args(["-s", "emulator-5554", "emu", "kill"])
            .output();
    }
    // Also kill via taskkill by name
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/IM", "emulator.exe"])
        .output();
    let _ = window.emit("log", format!("⏹ Stop signal sent to \"{}\"", name));
    CommandResult { ok: true, error: None, output: None }
}

// ─── Open Folder ─────────────────────────────────────────────────────────────
#[tauri::command]
pub fn open_folder(folder_path: String) -> CommandResult {
    let _ = std::process::Command::new("explorer.exe")
        .arg(&folder_path)
        .spawn();
    CommandResult { ok: true, error: None, output: None }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SdkPackageInfo {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub version: String,
    pub installed: bool,
    pub category: String,
}

#[tauri::command]
pub async fn fetch_sdk_packages(window: Window) -> Result<Vec<SdkPackageInfo>, String> {
    let bat = cmdline_dir().join("bin").join("sdkmanager.bat");
    let env = build_env();

    let cmd_str = format!("& '{}' --list", bat.display());

    let child = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &cmd_str])
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

                // FILTER: Exclude raw codenames (keep clean numeric releases)
                let id_lower = id.to_lowercase();
                if id_lower.contains("canary") || id_lower.contains("cinnamonbun") {
                    continue;
                }

                let id_split: Vec<&str> = id.split(';').collect();
                if id_split.len() >= 4 {
                    // Extract and normalize the base API level (e.g. "android-37.0" -> "37")
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
        // FILTER: For 16 KB builds, only allow them if they are the latest version (to show previews correctly)
        // This filters out all old 16KB duplicates for Android 14/15, but keeps Android 17 preview 16KB packages.
        if c.is_16k && c.api_level != max_api_level {
            continue;
        }

        // Classify as Beta/Preview if:
        // 1. It is the highest API level dynamically found in the list (e.g. Android 17 / API 37)
        // 2. OR it is a 16 KB page size build (which is experimental, not for daily use)
        // 3. OR the package ID explicitly contains preview/beta tags.
        // We do not check description strings for older API levels (like Android 14 XR developer preview)
        // so that they stay grouped under Stable where they belong.
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

        // Determine friendly android version name dynamically:
        // Starting with API 33 (Android 13), the formula is API - 20 (e.g. 33 -> 13, 34 -> 14, 35 -> 15, 36 -> 16, 37 -> 17...)
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

        packages.push(SdkPackageInfo {
            id: c.id,
            name,
            desc: c.desc,
            version: c.version,
            installed: c.is_installed,
            category,
        });
    }

    // Deduplicate and prioritize installed status
    let mut unique_packages: HashMap<String, SdkPackageInfo> = HashMap::new();
    for p in packages {
        if let Some(existing) = unique_packages.get_mut(&p.id) {
            if p.installed {
                existing.installed = true;
            }
        } else {
            unique_packages.insert(p.id.clone(), p);
        }
    }

    let mut result: Vec<SdkPackageInfo> = unique_packages.into_values().collect();
    // Sort so newest API levels are at the top
    result.sort_by(|a, b| b.id.cmp(&a.id));
    
    // Log success
    let _ = window.emit("log", format!("✅ Fetched {} system images from Google repositories.", result.len()));
    Ok(result)
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

    // Clamp RAM and Cores based on device type to prevent boot loops and memory mapping crashes
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
    
    if !updated_cores {
        new_lines.push(format!("hw.cpu.ncore={}", final_cores));
    }
    if !updated_ram {
        new_lines.push(format!("hw.ramSize={}", final_ram));
    }
    if !updated_gpu {
        new_lines.push(format!("hw.gpu.mode={}", gpu));
    }
    if !updated_heap {
        new_lines.push(format!("vm.heapSize={}", heap_size));
    }
    if !is_wear && resolution.is_some() {
        if !updated_width {
            new_lines.push(format!("hw.lcd.width={}", w));
        }
        if !updated_height {
            new_lines.push(format!("hw.lcd.height={}", h));
        }
        if !updated_skin {
            new_lines.push(format!("skin.name={}", resolution.as_ref().unwrap()));
        }
    }
    if !is_wear && dpi.is_some() && !updated_density {
        new_lines.push(format!("hw.lcd.density={}", density_val));
    }
    
    match std::fs::write(&config_path, new_lines.join("\n")) {
        Ok(_) => CommandResult { ok: true, error: None, output: None },
        Err(e) => CommandResult { ok: false, error: Some(e.to_string()), output: None },
    }
}

// ─── Optimize Guest Apps ──────────────────────────────────────────────────────
#[tauri::command]
pub async fn optimize_guest_apps(window: Window) -> CommandResult {
    let adb_exe = sdk_dir().join("platform-tools").join("adb.exe");
    if !adb_exe.exists() {
        return CommandResult { ok: false, error: Some("adb.exe not found".to_string()), output: None };
    }

    let _ = window.emit("log", "🔍 Scanning user-installed packages inside emulator...");

    // Get list of user installed packages (-3 option lists third party packages only)
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
        let _ = window.emit("log", "ℹ️ No third-party apps or games found to optimize.");
        return CommandResult { ok: true, error: None, output: Some("No third-party apps found".into()) };
    }

    let total = packages.len();
    let _ = window.emit("log", format!("⚡ Found {} apps/games. Starting Ahead-Of-Time (AOT) compilation...", total));

    for (idx, pkg) in packages.iter().enumerate() {
        let _ = window.emit("log", format!("⏳ [{}/{}] Optimizing compilation: {}...", idx + 1, total, pkg));
        
        // Execute compiler opt
        let _ = std::process::Command::new(&adb_exe)
            .args(["-s", "emulator-5554", "shell", "cmd", "package", "compile", "-m", "speed", "-f", pkg])
            .output();
    }

    let _ = window.emit("log", "✅ All user apps and games fully compiled to native machine code!");
    CommandResult { ok: true, error: None, output: Some("Optimization completed successfully".into()) }
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

