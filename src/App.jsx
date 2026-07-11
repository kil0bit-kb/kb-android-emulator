import { useState, useEffect, useCallback } from 'react'
import { useToast, ToastContainer, ConsoleLog, Spinner, ProgressBar } from './components/UI.jsx'
import { DeviceCard, CreateAvdDialog, EditAvdDialog } from './components/Devices.jsx'
import { SdkManager } from './components/SdkManager.jsx'
import { GpuSettings } from './components/GpuSettings.jsx'
import { Settings } from './components/Settings.jsx'
import * as api from './api.js'
import { useTranslation } from 'react-i18next'
import { 
  Sliders, 
  Package, 
  Smartphone, 
  Cpu, 
  Terminal, 
  Settings as SettingsIcon,
  RefreshCw, 
  Zap, 
  Plus, 
  AlertTriangle, 
  Coffee, 
  Wrench, 
  Bot,
  Play,
  Pause,
  CheckCircle2,
  Trash2,
  Download,
  Info
} from 'lucide-react'
import './index.css'

export default function App() {
  const { toasts, toast } = useToast()
  const [page, setPage] = useState('setup')
  const [logs, setLogs] = useState([])
  const [emulatorLogs, setEmulatorLogs] = useState({})

  const { t, i18n } = useTranslation()

  const changeLang = (newLang) => {
    i18n.changeLanguage(newLang)
    localStorage.setItem('app_lang', newLang)
  }

  const NAV = [
    { id: 'setup',   icon: <Sliders size={14} />,  label: t('nav_setup') },
    { id: 'sdk',     icon: <Package size={14} />, label: t('nav_sdk') },
    { id: 'devices', icon: <Smartphone size={14} />, label: t('nav_devices') },
    { id: 'gpu',     icon: <Cpu size={14} />, label: t('nav_gpu') },
    { id: 'logs',    icon: <Terminal size={14} />, label: t('nav_logs') },
    { id: 'settings',icon: <SettingsIcon size={14} />,  label: t('nav_settings') },
  ]
  const [logCapture, setLogCapture] = useState(
    localStorage.getItem('emulator_log_capture') !== 'false'
  )
  const [avds, setAvds] = useState([])
  const [appVersion, setAppVersion] = useState('0.1.0-beta')
  
  useEffect(() => {
    api.getAppVersion().then(setAppVersion).catch(e => console.error(e))
  }, [])

  const [showCreate, setShowCreate] = useState(false)
  const [editingAvd, setEditingAvd] = useState(null)
  const [status, setStatus] = useState(null)
  const [progress, setProgress] = useState({})
  const [installing, setInstalling] = useState({})
  const [sdkProgress, setSdkProgress] = useState({})
  const [sdkInstalling, setSdkInstalling] = useState({})
  const [sdkErrors, setSdkErrors] = useState({})
  const [gpus, setGpus] = useState([])
  const [hypervisor, setHypervisor] = useState(null)
  const [sysInfo, setSysInfo] = useState(null)
  const [loadingHardware, setLoadingHardware] = useState(false)
  const [optimizing, setOptimizing] = useState(false)

  // ── Listen to Tauri backend events ─────────────────────────────────────────
  useEffect(() => {
    const offLog = api.on('log', (line) => {
      setLogs(l => [...l.slice(-500), line])
    })
    const offProgress = api.on('progress', (data) => {
      setProgress(p => ({ ...p, [data.task]: data.pct }))
      setSdkProgress(p => ({ ...p, [data.task]: data.pct }))
    })
    const offEmulatorLog = api.on('emulator-log', ({ name, line }) => {
      // Only store if capture is enabled — read from localStorage for latest value
      if (localStorage.getItem('emulator_log_capture') === 'false') return
      setEmulatorLogs(prev => ({
        ...prev,
        [name]: [...(prev[name] || []).slice(-300), line.trim()]
      }))
    })
    const offEmulatorExit = api.on('emulator-exit', ({ name }) => {
      toast(`Emulator "${name}" stopped`, 'info')
      refreshAvds()
    })
    return () => { offLog?.(); offProgress?.(); offEmulatorLog?.(); offEmulatorExit?.() }
  }, [])

  // ── Load status & devices on mount & poll running state ─────────────────────
  useEffect(() => {
    refreshStatus()
    refreshAvds()

    const timer = setInterval(refreshAvds, 3000)
    return () => clearInterval(timer)
  }, [])

  // Trigger hardware detection on-demand when user opens creation/editing modals
  useEffect(() => {
    if ((showCreate || editingAvd) && !sysInfo) {
      refreshHardware()
    }
  }, [showCreate, editingAvd, sysInfo])

  const refreshHardware = async () => {
    setLoadingHardware(true)
    try {
      const [g, h, s] = await Promise.all([
        api.detectGpus(),
        api.checkHypervisor(),
        api.getSystemInfo()
      ])
      setGpus(g)
      setHypervisor(h)
      setSysInfo(s)
    } catch (e) {
      console.error("Failed to detect hardware:", e)
    }
    setLoadingHardware(false)
  }

  const refreshStatus = async () => {
    const s = await api.checkInstallStatus()
    setStatus(s)
  }

  const refreshAvds = async () => {
    const list = await api.listAvds()
    setAvds(Array.isArray(list) ? list : [])
  }

  const handleLaunch = async (name, wipeData = false) => {
    const avd = avds.find(a => a.name === name)
    const accel = localStorage.getItem('emulator_accel') || 'whpx'
    const gpuMode = avd?.gpu || localStorage.getItem('emulator_gpu') || 'host'
    let quickBootVal = localStorage.getItem(`emulator_quick_boot_${name}`)
    if (quickBootVal === null) {
      quickBootVal = localStorage.getItem('emulator_quick_boot')
    }
    const quickBoot = quickBootVal === 'true'

    let bootAnimVal = localStorage.getItem(`emulator_boot_anim_${name}`)
    if (bootAnimVal === null) {
      bootAnimVal = localStorage.getItem('emulator_boot_anim')
    }
    const bootAnim = bootAnimVal !== 'false'

    const noCamera = localStorage.getItem('emulator_perf_no_camera') === 'true'
    const noGps = localStorage.getItem('emulator_perf_no_gps') === 'true'
    const noBluetooth = localStorage.getItem('emulator_perf_no_bluetooth') === 'true'

    const readOnly = localStorage.getItem(`emulator_ultra_gaming_${name}`) === 'true'
    
    let speedModeVal = localStorage.getItem(`emulator_speed_mode_${name}`)
    if (speedModeVal === null) {
      speedModeVal = localStorage.getItem('emulator_speed_mode')
    }
    const speedMode = speedModeVal !== 'false'

    const rawLaunch = localStorage.getItem(`emulator_raw_launch_${name}`) === 'true'

    const result = await api.launchAvd({ 
      name, 
      gpuMode, 
      accel, 
      quickBoot, 
      bootAnim,
      noCamera,
      noGps,
      noBluetooth,
      readOnly,
      wipeData,
      speedMode,
      rawLaunch,
    })
    if (result.ok) {
      toast(`Launching "${name}"...`, 'info')
      setTimeout(refreshAvds, 500)
    } else {
      toast(result.error, 'error')
    }
  }

  const handleStop = async (name) => {
    const result = await api.stopAvd({ name })
    if (result.ok) {
      toast(`Stopped "${name}"`, 'info')
      setTimeout(refreshAvds, 500)
    }
  }

  const handleDelete = async (name) => {
    const result = await api.deleteAvd({ name })
    if (result.ok) {
      toast(`Deleted "${name}"`, 'success')
      refreshAvds()
    }
  }

  const handleOptimizeApps = async () => {
    setOptimizing(true)
    toast('Starting AOT compilation for all user apps...', 'info')
    try {
      const r = await api.optimizeGuestApps()
      if (r.ok) {
        toast('Optimization completed!', 'success')
      } else {
        toast(`Optimization failed: ${r.error || 'make sure emulator is running'}`, 'error')
      }
    } catch (e) {
      toast(`Error: ${e}`, 'error')
    }
    setOptimizing(false)
  }

  const installJdk = async () => {
    setInstalling(s => ({ ...s, jdk: true }))
    const r = await api.installJdk()
    setInstalling(s => ({ ...s, jdk: false }))
    toast(r.ok ? 'JDK installed!' : `JDK failed: ${r.error}`, r.ok ? 'success' : 'error')
    refreshStatus()
  }

  const installCmdline = async () => {
    setInstalling(s => ({ ...s, cmdline: true }))
    const r = await api.installCmdlineTools()
    setInstalling(s => ({ ...s, cmdline: false }))
    toast(r.ok ? 'cmdline-tools installed!' : `Failed: ${r.error}`, r.ok ? 'success' : 'error')
    refreshStatus()
  }

  const allReady = status?.jdk_installed && status?.cmdline_installed && status?.emulator_installed

  const StatusDot = ({ ok }) => (
    <span className={`badge ${ok ? 'badge-ok' : 'badge-warn'}`}>
      {ok ? (
        <span className="flex items-center gap-1">
          <CheckCircle2 size={10} />
          Installed
        </span>
      ) : (
        <span className="flex items-center gap-1">
          <AlertTriangle size={10} />
          Not installed
        </span>
      )}
    </span>
  )

  return (
    <div className="app-shell">
      {/* ── Title Bar ── */}
      <div className="titlebar">
        <div className="titlebar-logo">
          <img src="/icon.png" alt="Logo" style={{ width: '20px', height: '20px', borderRadius: '4px', objectFit: 'contain' }} />
          <span className="app-name" style={{ marginLeft: '8px' }}>KB Android Emulator</span>
          <span style={{ fontSize: '9px', opacity: 0.5, marginLeft: '6px', background: 'rgba(255,255,255,0.08)', padding: '2px 6px', borderRadius: '4px', letterSpacing: '0.5px' }}>
            v{appVersion}
          </span>
        </div>
        <div className="titlebar-spacer" />

        {status && (
          <div className="flex gap-2 items-center" style={{ fontSize: 11 }}>
            {allReady
              ? <span className="badge badge-ok flex items-center gap-1"><CheckCircle2 size={10} /> {t('ready')}</span>
              : <span className="badge badge-warn flex items-center gap-1"><AlertTriangle size={10} /> {t('setup_required')}</span>
            }
            {avds.filter(a => a.running).length > 0 && (
              <span className="badge badge-running flex items-center gap-1">
                <span className="dot" />
                {avds.filter(a => a.running).length} {t('running')}
              </span>
            )}
            {/* Log capture quick-toggle — always accessible from any page */}
            <span
              id="btn-log-capture-toggle"
              className={`badge ${logCapture ? 'badge-ok' : 'badge-warn'} flex items-center gap-1`}
              onClick={() => {
                const next = !logCapture
                setLogCapture(next)
                localStorage.setItem('emulator_log_capture', String(next))
                toast(next ? 'Emulator log capture ON' : 'Emulator log capture OFF', 'info')
              }}
              title={logCapture ? 'Log capture ON — click to pause' : 'Log capture OFF — click to resume'}
              style={{ cursor: 'pointer', userSelect: 'none' }}
            >
              {logCapture ? <span className="flex items-center gap-1"><Play size={10} />Log ON</span> : <span className="flex items-center gap-1"><Pause size={10} />Log OFF</span>}
            </span>
          </div>
        )}

        <div className="titlebar-controls">
          <button className="titlebar-btn min" onClick={() => api.minimizeWindow()} title="Minimize" />
          <button className="titlebar-btn max" onClick={() => api.maximizeWindow()} title="Maximize" />
          <button className="titlebar-btn close" onClick={() => api.closeWindow()} title="Close" />
        </div>
      </div>

      <div className="body-split">
        {/* ── Sidebar ── */}
        <aside className="sidebar">
          <div className="sidebar-section-label">Navigation</div>
          {NAV.map(n => (
            <div key={n.id}
              className={`sidebar-item ${page === n.id ? 'active' : ''}`}
              onClick={() => {
                setPage(n.id)
                if (n.id === 'gpu' && !sysInfo) {
                  refreshHardware()
                }
              }}
              id={`nav-${n.id}`}
            >
              <span className="icon">{n.icon}</span>
              <span>{n.label}</span>
            </div>
          ))}
          <div className="sidebar-footer">
            <a href="https://github.com/kil0bit-kb/kb-android-emulator" target="_blank" rel="noopener noreferrer" className='text-muted text-sm'>KB Android Emulator</a><br />
            <span className="text-muted" style={{ fontSize: 10 }}>Powered by Google Android SDK, Tauri & React</span>
          </div>
        </aside>

        {/* ── Main Content ── */}
        <main className="main-content">

          {/* ─── Devices Page ─── */}
          {page === 'devices' && (
            <div className="page">
              <div className="page-header flex items-center justify-between">
                <div>
                  <h1 className="page-title flex items-center gap-2">
                    <Smartphone size={20} />
                    {t('devices_title')}
                  </h1>
                  <p className="page-subtitle">{t('devices_subtitle')}</p>
                </div>
                <div className="flex gap-2">
                  <button className="btn btn-ghost btn-sm flex items-center gap-1" onClick={refreshAvds}>
                    <RefreshCw size={12} />
                    {t('devices_refresh')}
                  </button>
                  <button 
                    className="btn btn-ghost btn-sm flex items-center gap-1" 
                    onClick={handleOptimizeApps}
                    disabled={optimizing || avds.filter(a => a.running).length === 0}
                    title="Pre-compiles installed games/apps to native machine code to eliminate JIT stuttering. Requires a running emulator."
                  >
                    <Zap size={12} />
                    {optimizing ? 'Optimizing...' : t('devices_optimize')}
                  </button>
                  <button className="btn btn-primary flex items-center gap-1" onClick={() => setShowCreate(true)}
                    disabled={!allReady} id="btn-create-device">
                    <Plus size={12} />
                    {t('devices_new')}
                  </button>
                </div>
              </div>

              {!allReady && (
                <div className="alert alert-warn flex items-center gap-2" style={{ marginBottom: 20 }}>
                  <AlertTriangle size={14} style={{ flexShrink: 0 }} />
                  <span>Setup is incomplete. Go to <strong>Setup / Install</strong> first.</span>
                </div>
              )}

              {avds.length === 0 ? (
                <div className="empty-state">
                  <div className="empty-icon"><Smartphone size={40} style={{ opacity: 0.25 }} /></div>
                  <div className="empty-title">{t('devices_empty_title')}</div>
                  <div className="empty-desc">
                    {t('devices_empty_desc')}
                  </div>
                  <button className="btn btn-primary flex items-center gap-1" onClick={() => setShowCreate(true)} disabled={!allReady}>
                    <Plus size={12} />
                    {t('devices_create_first')}
                  </button>
                </div>
              ) : (
                <div className="grid-auto">
                  {avds.map(avd => (
                    <DeviceCard key={avd.name} avd={avd}
                      onLaunch={handleLaunch}
                      onStop={handleStop}
                      onDelete={handleDelete}
                      onEdit={(target) => setEditingAvd(target)}
                      logs={emulatorLogs}
                    />
                  ))}
                </div>
              )}
            </div>
          )}

          {/* ─── SDK Manager Page ─── */}
          {page === 'sdk' && (
            <div className="page">
              <div className="page-header">
                <h1 className="page-title flex items-center gap-2">
                  <Package size={20} />
                  {t('sdk_title')}
                </h1>
                <p className="page-subtitle">{t('sdk_subtitle')}</p>
              </div>
              {status?.cmdline_installed
                ? <SdkManager 
                    logs={logs} 
                    status={status} 
                    refreshStatus={refreshStatus}
                    progress={sdkProgress}
                    setProgress={setSdkProgress}
                    installing={sdkInstalling}
                    setInstalling={setSdkInstalling}
                    errors={sdkErrors}
                    setErrors={setSdkErrors}
                  />
                : <div className="alert alert-warn flex items-center gap-2">
                    <AlertTriangle size={14} style={{ flexShrink: 0 }} />
                    <span>Please install cmdline-tools first from the <strong>Setup</strong> page.</span>
                  </div>
              }
            </div>
          )}

          {/* ─── GPU & Performance Page ─── */}
          {page === 'gpu' && (
            <div className="page">
              <div className="page-header">
                <h1 className="page-title flex items-center gap-2">
                  <Cpu size={20} />
                  {t('gpu_title')}
                </h1>
                <p className="page-subtitle">{t('gpu_subtitle')}</p>
              </div>
              <GpuSettings 
                toast={toast} 
                gpus={gpus} 
                hypervisor={hypervisor} 
                sysInfo={sysInfo} 
                loading={loadingHardware} 
                onRescan={refreshHardware}
                emojisEnabled={emojisEnabled}
              />
            </div>
          )}

          {/* ─── Setup / Install Page ─── */}
          {page === 'setup' && (
            <div className="page">
              <div className="page-header">
                <h1 className="page-title flex items-center gap-2">
                  <Sliders size={20} />
                  {t('setup_title')}
                </h1>
                <p className="page-subtitle">{t('setup_subtitle')}</p>
              </div>

              <div className="alert alert-info flex items-start gap-2" style={{ marginBottom: 24 }}>
                <Info size={14} style={{ flexShrink: 0, marginTop: 2 }} />
                <span>{t('setup_warn')}</span>
              </div>

              <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
                {[
                  {
                    step: 1, key: 'jdk', icon: <Coffee size={20} />,
                    title: 'Portable OpenJDK 21',
                    desc: 'Eclipse Temurin JDK 21 — required to run sdkmanager and avdmanager. Stored locally, no global install.',
                    size: '~180 MB',
                    installed: status?.jdk_installed,
                    onInstall: installJdk,
                    loading: installing.jdk,
                    requires: false,
                  },
                  {
                    step: 2, key: 'cmdline', icon: <Wrench size={20} />,
                    title: 'Android Command Line Tools',
                    desc: 'Official Google sdkmanager & avdmanager — to install emulator, platform tools, and system images.',
                    size: '~130 MB',
                    installed: status?.cmdline_installed,
                    onInstall: installCmdline,
                    loading: installing.cmdline,
                    requires: !status?.jdk_installed,
                  },
                  {
                    step: 3, key: 'emulator', icon: <Bot size={20} />,
                    title: 'Android Emulator Engine',
                    desc: 'Install via SDK Manager → "emulator" package once cmdline-tools are ready.',
                    size: '~300 MB',
                    installed: status?.emulator_installed,
                    onInstall: null,
                    requires: false,
                  },
                ].map(item => (
                  <div key={item.key} className="card">
                    <div className="flex items-center gap-3" style={{ marginBottom: 12 }}>
                      <div className="stat-icon" style={{
                        background: item.installed ? 'rgba(16,185,129,0.15)' : 'rgba(120,80,255,0.15)',
                        width: 48, height: 48,
                      }}>{item.icon}</div>
                      <div style={{ flex: 1 }}>
                        <div className="flex items-center gap-2">
                          <span style={{ fontWeight: 700, color: 'var(--text-primary)' }}>
                            Step {item.step}: {item.title}
                          </span>
                          <StatusDot ok={item.installed} />
                        </div>
                        <div style={{ fontSize: 12, color: 'var(--text-muted)', marginTop: 3 }}>{item.desc}</div>
                        <div className="badge badge-gpu flex items-center gap-1" style={{ marginTop: 6 }}>
                          <Package size={10} />
                          {item.size}
                        </div>
                      </div>
                      {item.onInstall && (
                        <button
                          className={`btn ${item.installed ? 'btn-ghost' : 'btn-primary'} btn-sm flex items-center gap-1`}
                          onClick={item.onInstall}
                          disabled={item.loading || item.requires || item.installed}
                          id={`btn-install-${item.key}`}
                        >
                          {item.loading ? <><Spinner size={13} />Downloading…</> : item.installed ? 'Installed' : <><Download size={12} />Download & Install</>}
                        </button>
                      )}
                      {!item.onInstall && !item.installed && (
                        <button className="btn btn-ghost btn-sm" onClick={() => setPage('sdk')}>
                          Go to SDK Manager →
                        </button>
                      )}
                    </div>
                    {item.loading && progress[item.key] !== undefined && (
                      <div>
                        <div className="flex justify-between text-sm text-muted" style={{ marginBottom: 4 }}>
                          <span>Downloading...</span>
                          <span>{progress[item.key]}%</span>
                        </div>
                        <ProgressBar value={progress[item.key]} />
                      </div>
                    )}
                    {item.requires && (
                      <div className="alert alert-warn mt-2 flex items-center gap-2" style={{ fontSize: 12 }}>
                        <AlertTriangle size={12} style={{ flexShrink: 0 }} />
                        <span>Requires Step {item.step - 1} to be installed first.</span>
                      </div>
                    )}
                  </div>
                ))}

                {allReady && (
                  <div className="alert alert-success flex items-start gap-2">
                    <CheckCircle2 size={14} style={{ flexShrink: 0, marginTop: 2 }} />
                    <span>All components installed! Go to <strong>SDK Manager</strong> to install a system image, then <strong>My Devices</strong> to create your first device.</span>
                  </div>
                )}
              </div>
            </div>
          )}

          {/* ─── Settings Page ─── */}
          {page === 'settings' && (
            <Settings 
              toast={toast}
            />
          )}

          {/* ─── Logs Page ─── */}
          {page === 'logs' && (
            <div className="page">
              <div className="page-header flex items-center justify-between">
                <div>
                  <h1 className="page-title flex items-center gap-2">
                    <Terminal size={20} />
                    {t('logs_title')}
                  </h1>
                  <p className="page-subtitle">{t('logs_subtitle')}</p>
                </div>
                <button className="btn btn-ghost btn-sm flex items-center gap-1" onClick={() => setLogs([])}>
                  <Trash2 size={12} />
                  {t('logs_clear')}
                </button>
              </div>
              <ConsoleLog lines={logs} />

              {/* ── Emulator Log Section ── */}
              <div className="divider" style={{ margin: '20px 0 12px' }} />
              <div className="flex items-center justify-between" style={{ marginBottom: 12 }}>
                <div className="section-title flex items-center gap-2" style={{ margin: 0 }}>
                  <Terminal size={14} />
                  {t('logs_capture_title')}
                </div>
                <div className="flex gap-2">
                  {Object.keys(emulatorLogs).length > 0 && (
                    <button
                      className="btn btn-ghost btn-sm flex items-center gap-1"
                      onClick={() => setEmulatorLogs({})}
                      title="Clear all emulator log output"
                    >
                      <Trash2 size={12} />
                      Clear Log
                    </button>
                  )}
                  <button
                    id="btn-log-capture-toggle"
                    className={`btn btn-sm flex items-center gap-1 ${logCapture ? 'btn-primary' : 'btn-ghost'}`}
                    onClick={() => {
                      const next = !logCapture
                      setLogCapture(next)
                      localStorage.setItem('emulator_log_capture', String(next))
                    }}
                    title={logCapture ? 'Click to pause log capture' : 'Click to resume log capture'}
                  >
                    {logCapture ? <span className="flex items-center gap-1"><Play size={12} />Capturing</span> : <span className="flex items-center gap-1"><Pause size={12} />Paused</span>}
                  </button>
                </div>
              </div>

              {!logCapture && (
                <div className="alert alert-warn flex items-center gap-2" style={{ marginBottom: 12, fontSize: 13 }}>
                  <Pause size={14} style={{ flexShrink: 0 }} />
                  <span>{t('logs_paused_warn')}</span>
                </div>
              )}

              {Object.keys(emulatorLogs).length > 0 ? (
                Object.entries(emulatorLogs).map(([name, lines]) => (
                  <div key={name} style={{ marginBottom: 16 }}>
                    <div className="card-title flex items-center gap-2" style={{ marginBottom: 8 }}>
                      <Smartphone size={14} />
                      {name}
                    </div>
                    <ConsoleLog lines={lines} />
                  </div>
                ))
              ) : (
                <div style={{ color: 'var(--text-muted)', fontSize: 13, padding: '12px 0' }}>
                  {logCapture ? t('logs_empty') : 'Log capture is paused.'}
                </div>
              )}
            </div>
          )}

        </main>
      </div>

      {/* ─── Create AVD Dialog ─── */}
      {showCreate && (
        <CreateAvdDialog
          gpus={gpus}
          logs={logs}
          status={status}
          onClose={() => setShowCreate(false)}
          onCreated={() => {
            setShowCreate(false)
            toast('✅ Device created successfully!', 'success')
            refreshAvds()
          }}
        />
      )}
      {editingAvd && (
        <EditAvdDialog
          avd={editingAvd}
          gpus={gpus}
          onClose={() => setEditingAvd(null)}
          onSaved={() => {
            setEditingAvd(null)
            toast('✅ Device configuration updated!', 'success')
            refreshAvds()
          }}
        />
      )}
      <ToastContainer toasts={toasts} />
    </div>
  )
}
