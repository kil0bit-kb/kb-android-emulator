use super::paths::emulator_dir;
use super::types::{CommandResult, HypervisorStatus};

// ─── Check Hypervisor ─────────────────────────────────────────────────────────
#[tauri::command]
pub fn check_hypervisor() -> HypervisorStatus {
    check_hypervisor_impl()
}

#[cfg(windows)]
fn check_hypervisor_impl() -> HypervisorStatus {
    use std::os::windows::process::CommandExt;
    // Check if Hypervisor is active (WSL2 / Hyper-V / WHPX is running).
    // This query is instant and does NOT require Administrator elevation.
    let hypervisor_present_out = std::process::Command::new("powershell")
        .creation_flags(0x08000000)
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
        return HypervisorStatus {
            whpx_enabled: true,
            vtx_enabled: true,
        };
    }

    // Fallback: If hypervisor is NOT present, query if VT-x/AMD-V is enabled in BIOS
    let vtx = std::process::Command::new("powershell")
        .creation_flags(0x08000000)
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

#[cfg(not(windows))]
fn check_hypervisor_impl() -> HypervisorStatus {
    // On Linux check /proc/cpuinfo for vmx (Intel VT-x) or svm (AMD-V)
    let vtx_enabled = std::fs::read_to_string("/proc/cpuinfo")
        .map(|s| s.contains("vmx") || s.contains("svm"))
        .unwrap_or(false);

    // Check if KVM is available as hypervisor
    let whpx_enabled = std::path::Path::new("/dev/kvm").exists();

    HypervisorStatus { whpx_enabled, vtx_enabled }
}

// ─── Enable WHPX ─────────────────────────────────────────────────────────────
#[tauri::command]
pub fn enable_whpx() -> CommandResult {
    enable_whpx_impl()
}

#[cfg(windows)]
fn enable_whpx_impl() -> CommandResult {
    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("powershell")
        .creation_flags(0x08000000)
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

#[cfg(not(windows))]
fn enable_whpx_impl() -> CommandResult {
    CommandResult {
        ok: false,
        error: Some("WHPX is a Windows-only feature. On Linux, ensure KVM is enabled: 'sudo modprobe kvm kvm_intel' (or kvm_amd).".to_string()),
        output: None,
    }
}

// ─── Set GPU Preference (Windows Registry) ────────────────────────────────────
#[tauri::command]
pub fn set_gpu_preference(_gpu_index: usize, gpu_preference: u32) -> CommandResult {
    set_gpu_preference_impl(_gpu_index, gpu_preference)
}

#[cfg(windows)]
fn set_gpu_preference_impl(_gpu_index: usize, gpu_preference: u32) -> CommandResult {
    use winreg::enums::*;
    use winreg::RegKey;
    use std::path::PathBuf;

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

#[cfg(not(windows))]
fn set_gpu_preference_impl(_gpu_index: usize, _gpu_preference: u32) -> CommandResult {
    CommandResult {
        ok: false,
        error: Some("GPU registry preference is a Windows-only feature (DirectX/DXGI). On Linux, GPU selection is handled via MESA_VK_DEVICE_SELECT or DRI_PRIME environment variables.".to_string()),
        output: None,
    }
}
