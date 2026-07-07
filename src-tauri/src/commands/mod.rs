// commands/mod.rs — Public facade for all backend commands
//
// Sub-modules (each focused on a single domain):
//   types.rs      — Shared data structures
//   paths.rs      — SDK/JDK path helpers and build_env()
//   system_info.rs — get_system_info, detect_gpus (uses sysinfo crate)
//   hypervisor.rs  — check_hypervisor, enable_whpx, set_gpu_preference (cfg-gated)
//   install.rs    — JDK/cmdline-tools install, sdkmanager wrappers
//   avd.rs        — AVD list/create/delete/update
//   launch.rs     — launch_avd, stop_avd, optimize_guest_apps (cfg-gated priority)
//   window.rs     — Window controls, open_folder, get_app_version

pub mod types;
pub mod paths;
pub mod system_info;
pub mod hypervisor;
pub mod install;
pub mod avd;
pub mod launch;
pub mod window;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

// ─── Shared global state ──────────────────────────────────────────────────────
pub static RUNNING_AVDS: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();

// ─── Shared macros ────────────────────────────────────────────────────────────
/// Create a std::process::Command with CREATE_NO_WINDOW on Windows.
#[macro_export]
macro_rules! new_command {
    ($cmd:expr) => {{
        let mut c = std::process::Command::new($cmd);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        c
    }};
}

/// Create a tokio::process::Command with CREATE_NO_WINDOW on Windows.
#[macro_export]
macro_rules! new_tokio_command {
    ($cmd:expr) => {{
        let mut c = tokio::process::Command::new($cmd);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        c
    }};
}

// ─── Shared re-export for setup ──────────────────────────────────────────────
// main.rs calls commands::ensure_dirs directly
pub use paths::ensure_dirs;
