use super::types::{GpuInfo, SystemInfo};

// ─── System Info ──────────────────────────────────────────────────────────────
#[tauri::command]
pub fn get_system_info() -> SystemInfo {
    SystemInfo {
        total_ram: total_memory_mb(),
        free_ram: free_memory_mb(),
        cpu_count: num_cpus(),
        cpu_model: cpu_model(),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

pub fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

// Use the sysinfo crate for accurate cross-platform memory reporting.
// The previous implementation used PowerShell with a hardcoded fallback of
// 8192 MB, which caused the 8 GB display bug on systems with more RAM.
pub fn total_memory_mb() -> u64 {
    use sysinfo::System;
    let sys = System::new_all();
    let bytes = sys.total_memory();
    // sysinfo reports in bytes
    bytes / (1024 * 1024)
}

pub fn free_memory_mb() -> u64 {
    use sysinfo::System;
    let sys = System::new_all();
    let bytes = sys.available_memory();
    bytes / (1024 * 1024)
}

pub fn cpu_model() -> String {
    use sysinfo::System;
    let sys = System::new_all();
    sys.cpus()
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string())
}

// ─── GPU Detection ────────────────────────────────────────────────────────────
#[tauri::command]
pub fn detect_gpus() -> Vec<GpuInfo> {
    detect_gpus_impl()
}

#[cfg(windows)]
fn detect_gpus_impl() -> Vec<GpuInfo> {
    use std::os::windows::process::CommandExt;
    // Use -Encoding UTF8 to avoid BOM / UTF-16 byte corruption in the parsed JSON.
    let out = std::process::Command::new("powershell")
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Encoding",
            "UTF8",
            "-Command",
            "Get-WmiObject Win32_VideoController | Select-Object Name, AdapterRAM | ConvertTo-Json",
        ])
        .output();

    let Ok(o) = out else { return vec![] };

    // Strip BOM (EF BB BF) if present before parsing
    let raw = String::from_utf8_lossy(&o.stdout);
    let text = raw.trim_start_matches('\u{feff}').trim();

    let json: serde_json::Value = serde_json::from_str(text).unwrap_or(serde_json::Value::Null);

    let arr = match json {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(_) => vec![json],
        _ => return vec![],
    };

    arr.into_iter()
        .enumerate()
        .map(|(i, g)| {
            // Strip any residual whitespace / control characters from the name
            let raw_name = g["Name"].as_str().unwrap_or("Unknown GPU");
            let name = raw_name
                .trim_matches(|c: char| c.is_control() || c == '\u{feff}')
                .trim()
                .to_string();
            let ram_bytes = g["AdapterRAM"].as_u64().unwrap_or(0);
            let vram = if ram_bytes > 0 {
                format!("{} MB", ram_bytes / (1024 * 1024))
            } else {
                "Unknown".to_string()
            };
            let name_lower = name.to_lowercase();
            let is_dedicated = name_lower.contains("nvidia")
                || name_lower.contains("amd")
                || name_lower.contains("radeon")
                || name_lower.contains("geforce")
                || name_lower.contains("rtx")
                || name_lower.contains("gtx");
            GpuInfo { index: i, name, vram, is_dedicated }
        })
        .collect()
}

#[cfg(not(windows))]
fn detect_gpus_impl() -> Vec<GpuInfo> {
    // On Linux, attempt to read GPU info from lspci or return a minimal stub.
    let out = std::process::Command::new("lspci")
        .args(["-mm"])
        .output();

    let Ok(o) = out else {
        return vec![GpuInfo {
            index: 0,
            name: "GPU detection not available on this platform".to_string(),
            vram: "Unknown".to_string(),
            is_dedicated: false,
        }];
    };

    let text = String::from_utf8_lossy(&o.stdout);
    let mut gpus = vec![];
    for line in text.lines() {
        let lower = line.to_lowercase();
        if lower.contains("vga") || lower.contains("display") || lower.contains("3d") {
            // lspci -mm format: slot "class" "vendor" "device" ...
            let parts: Vec<&str> = line.splitn(5, '"').collect();
            let name = if parts.len() >= 4 {
                format!("{} {}", parts.get(3).unwrap_or(&""), parts.get(4).map(|s| s.trim_matches('"')).unwrap_or("")).trim().to_string()
            } else {
                line.to_string()
            };
            let is_dedicated = lower.contains("nvidia") || lower.contains("amd") || lower.contains("radeon");
            gpus.push(GpuInfo {
                index: gpus.len(),
                name,
                vram: "Unknown".to_string(),
                is_dedicated,
            });
        }
    }

    if gpus.is_empty() {
        gpus.push(GpuInfo {
            index: 0,
            name: "GPU detection unavailable".to_string(),
            vram: "Unknown".to_string(),
            is_dedicated: false,
        });
    }
    gpus
}
