/**
 * Tauri API bridge — wraps @tauri-apps/api invoke and event listener
 * so the rest of the React code works identically to before.
 */
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'

const appWindow = getCurrentWindow()

// ─── Window Controls ──────────────────────────────────────────────────────────
export const minimizeWindow = () => invoke('window_minimize')
export const maximizeWindow = () => invoke('window_maximize')
export const closeWindow    = () => invoke('window_close')

// ─── System ───────────────────────────────────────────────────────────────────
export const getSystemInfo      = () => invoke('get_system_info')
export const detectGpus         = () => invoke('detect_gpus')
export const setGpuPreference   = (opts) => invoke('set_gpu_preference', { gpuIndex: opts.gpuIndex, gpuPreference: opts.gpuPreference })
export const checkHypervisor    = () => invoke('check_hypervisor')
export const enableWhpx         = () => invoke('enable_whpx')
export const checkInstallStatus = () => invoke('check_install_status')

// ─── Downloads / SDK ──────────────────────────────────────────────────────────
export const installJdk          = () => invoke('install_jdk')
export const installCmdlineTools = () => invoke('install_cmdline_tools')
export const acceptLicenses      = () => invoke('accept_licenses')
export const installPackage      = (opts) => invoke('install_package', { packageId: opts.packageId })
export const fetchSdkPackages    = () => invoke('fetch_sdk_packages')

// ─── AVD Management ───────────────────────────────────────────────────────────
export const listAvds  = () => invoke('list_avds')
export const createAvd = (opts) => invoke('create_avd', { options: opts })
export const deleteAvd = (opts) => invoke('delete_avd', { name: opts.name })
export const launchAvd = (opts) => invoke('launch_avd', { 
  name: opts.name, 
  gpuMode: opts.gpuMode, 
  accel: opts.accel, 
  quickBoot: opts.quickBoot, 
  bootAnim: opts.bootAnim,
  noCamera: opts.noCamera,
  noGps: opts.noGps,
  noBluetooth: opts.noBluetooth,
  readOnly: opts.readOnly,
  wipeData: opts.wipeData
})
export const stopAvd   = (opts) => invoke('stop_avd', { name: opts.name })
export const openFolder = (opts) => invoke('open_folder', { folderPath: opts.folderPath })
export const updateAvdConfig = (opts) => invoke('update_avd_config', { 
  name: opts.name, 
  cores: opts.cores, 
  ram: opts.ram, 
  gpu: opts.gpu,
  resolution: opts.resolution,
  dpi: opts.dpi
})
export const optimizeGuestApps = () => invoke('optimize_guest_apps')
export const getAppVersion = () => invoke('get_app_version')

// ─── Event listeners (mirrors Electron's window.api.on) ─────────────────────
const _unlisten = {}

export function on(channel, cb) {
  let unlisten
  listen(channel, (event) => cb(event.payload)).then(u => {
    unlisten = u
  })
  return () => { if (unlisten) unlisten() }
}

export function off(channel) {
  if (_unlisten[channel]) { _unlisten[channel](); delete _unlisten[channel] }
}
