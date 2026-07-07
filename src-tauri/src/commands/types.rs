use serde::{Deserialize, Serialize};

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
    pub licenses_accepted: bool,
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
    pub raw_launch: Option<bool>,
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

#[derive(Serialize, Deserialize, Clone)]
pub struct SdkPackageInfo {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub version: String,
    pub installed: bool,
    pub category: String,
}
