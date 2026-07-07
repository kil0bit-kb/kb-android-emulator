use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ─── Path helpers ─────────────────────────────────────────────────────────────
pub fn sdk_base() -> PathBuf {
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

pub fn jdk_dir() -> PathBuf {
    sdk_base().join("jdk")
}
pub fn sdk_dir() -> PathBuf {
    sdk_base().join("sdk")
}
pub fn cmdline_dir() -> PathBuf {
    sdk_dir().join("cmdline-tools").join("latest")
}
pub fn emulator_dir() -> PathBuf {
    sdk_dir().join("emulator")
}
pub fn avd_dir() -> PathBuf {
    sdk_base().join("avd")
}

pub fn ensure_dirs(base: &PathBuf) {
    let _ = base; // we use sdk_base() internally
    for d in [sdk_base(), jdk_dir(), sdk_dir(), avd_dir()] {
        let _ = std::fs::create_dir_all(&d);
    }
}

pub fn get_java_exe() -> Option<PathBuf> {
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
    #[cfg(windows)]
    let java = entry.path().join("bin").join("java.exe");
    #[cfg(not(windows))]
    let java = entry.path().join("bin").join("java");
    if java.exists() {
        Some(java)
    } else {
        None
    }
}

pub fn get_java_home() -> Option<PathBuf> {
    let jdk = jdk_dir();
    let entry = std::fs::read_dir(&jdk)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().starts_with("jdk"))?;
    Some(entry.path())
}

pub fn build_env() -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();
    if let Some(jh) = get_java_home() {
        env.insert("JAVA_HOME".to_string(), jh.to_string_lossy().to_string());
        let bin = jh.join("bin").to_string_lossy().to_string();
        let path = env.get("PATH").cloned().unwrap_or_default();
        #[cfg(windows)]
        env.insert("PATH".to_string(), format!("{};{}", bin, path));
        #[cfg(not(windows))]
        env.insert("PATH".to_string(), format!("{}:{}", bin, path));
    }
    env.insert("ANDROID_SDK_ROOT".to_string(), sdk_dir().to_string_lossy().to_string());
    env.insert("ANDROID_AVD_HOME".to_string(), avd_dir().to_string_lossy().to_string());
    env.insert("ANDROID_EMULATOR_HOME".to_string(), avd_dir().to_string_lossy().to_string());
    
    // Low-latency WASAPI Audio Optimizations (Windows only — harmless to set on other OS)
    env.insert("QEMU_AUDIO_TIMER_PERIOD".to_string(), "0".to_string());
    env.insert("QEMU_WASAPI_BUF_SIZE".to_string(), "512".to_string());

    env
}
