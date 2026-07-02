import { useState } from 'react'
import { Spinner, ConsoleLog } from './UI.jsx'
import * as api from '../api.js'


const RESOLUTIONS = [
  { val: '1080x1920', label: 'Full HD 1080×1920 (Phone)' },
  { val: '1280x720',  label: 'HD 1280×720 (Phone)' },
  { val: '2340x1080', label: 'FHD+ 2340×1080 (Gaming)' },
  { val: '1920x1080', label: 'Full HD Landscape 1920×1080' },
  { val: '2560x1600', label: 'WQXGA 2560×1600 (Tablet)' },
]
const GPU_MODES = [
  { val: 'auto',     label: 'Auto (Recommended - Best balance for dGPU/iGPU)' },
  { val: 'host',     label: 'Hardware Accelerated (Force Graphics Card)' },
  { val: 'software', label: 'Software Emulation (Safe Mode / Diagnostics)' },
]

export function CreateAvdDialog({ onClose, onCreated, gpus, status }) {
  const installedImages = (status?.installed_packages || []).filter(pkg => pkg.startsWith("system-images;"))

  const getAndroidVersion = (pkgId) => {
    const parts = pkgId.split(';')
    if (parts.length < 2) return 'Device'
    const apiLevel = parseInt(parts[1].replace('android-', ''))
    if (isNaN(apiLevel)) return 'Device'
    
    const mapping = {
      37: '17',
      36: '16',
      35: '15',
      34: '14',
      33: '13',
      32: '12L',
      31: '12',
      30: '11',
      29: '10',
      28: '9',
    };
    return mapping[apiLevel] || `API_${apiLevel}`
  }

  const [randId] = useState(() => Math.floor(1000 + Math.random() * 9000))

  const [form, setForm] = useState(() => {
    const defaultGpu = 'auto';
    
    const initialImage = installedImages[0] || '';
    let initialName = `Device_${randId}`;
    if (initialImage) {
      const parts = initialImage.split(';')
      if (parts.length >= 2) {
        const apiLevel = parseInt(parts[1].replace('android-', ''))
        const mapping = {
          37: '17', 36: '16', 35: '15', 34: '14', 33: '13', 32: '12L', 31: '12', 30: '11', 29: '10', 28: '9'
        };
        const ver = mapping[apiLevel] || `API_${apiLevel}`;
        initialName = `Android_${ver}_Device_${randId}`;
      }
    }

    return {
      name: initialName,
      systemImage: initialImage,
      ram: 4096, cores: 4, storage: 8192,
      gpuMode: defaultGpu, screenResolution: '1280x720', dpi: 240,
      showQtSidebar: false,
    }
  })
  const [creating, setCreating] = useState(false)
  const set = (k, v) => setForm(f => ({ ...f, [k]: v }))

  const handleSystemImageChange = (val) => {
    const apiLower = val.toLowerCase();
    const isWear = apiLower.includes('wear');
    const isTv = apiLower.includes('tv') || apiLower.includes('google-tv');
    const isAuto = apiLower.includes('automotive');

    setForm(f => {
      const parts = f.name.split('_')
      const isDefaultPattern = f.name.startsWith('Device_') || 
                               (f.name.startsWith('Android_') && f.name.includes('_Device_')) ||
                               (f.name.startsWith('Watch_') && f.name.includes('_Device_')) ||
                               (f.name.startsWith('TV_') && f.name.includes('_Device_')) ||
                               (f.name.startsWith('Car_') && f.name.includes('_Device_'));
      
      let newName = f.name;
      if (isDefaultPattern) {
        const rand = parts[parts.length - 1] || String(randId);
        const ver = getAndroidVersion(val);
        newName = isWear ? `Watch_API_${ver}_Device_${rand}` :
                  isTv ? `TV_API_${ver}_Device_${rand}` :
                  isAuto ? `Car_API_${ver}_Device_${rand}` :
                  `Android_${ver}_Device_${rand}`;
      }
      
      return {
        ...f,
        name: newName,
        systemImage: val,
        ram: isWear ? 1024 : isTv ? 2048 : isAuto ? 4096 : 4096,
        cores: isWear ? 1 : isTv ? 2 : isAuto ? 4 : 4,
        screenResolution: isWear ? '390x390' : isTv ? '1920x1080' : '1280x720',
        dpi: isWear ? 320 : isTv ? 240 : 240,
      }
    })
  }

  const getFriendlyName = (id) => {
    const parts = id.split(';')
    if (parts.length >= 4) {
      const apiLevel = parts[1].replace('android-', '')
      const type = parts[2].replace('_', ' ').toUpperCase()
      const arch = parts[3]
      return `Android (API ${apiLevel}) · ${type} · ${arch}`
    }
    return id
  }

  const handleCreate = async () => {
    if (!form.name.trim() || !form.systemImage) return
    setCreating(true)
    const result = await api.createAvd({
      name: form.name,
      system_image: form.systemImage,
      ram: form.ram,
      cores: form.cores,
      storage: form.storage,
      gpu_mode: form.gpuMode,
      screen_resolution: form.screenResolution,
      dpi: form.dpi,
    })
    setCreating(false)
    if (result.ok) {
      localStorage.setItem(`emulator_qt_sidebar_${form.name}`, String(form.showQtSidebar))
      onCreated()
    }
  }

  return (
    <div className="modal-overlay" onClick={e => e.target === e.currentTarget && onClose()}>
      <div className="modal" style={{ width: 560 }}>
        <div className="modal-title"><span>📱</span><span>Create New Android Device</span></div>
        <div className="modal-form">
          <div className="form-group">
            <label className="form-label">Device Name</label>
            <input className="form-input" value={form.name} onChange={e => set('name', e.target.value)} placeholder="e.g. Pixel_9_Gaming" />
          </div>
          <div className="form-group">
            <label className="form-label">System Image</label>
            {installedImages.length === 0 ? (
              <div className="alert alert-danger" style={{ fontSize: 12 }}>
                ⚠️ <strong>No system images installed!</strong> Please go to the <strong>SDK Manager</strong> page and install at least one system image (phone, TV, etc.) first.
              </div>
            ) : (
              <select className="form-select" value={form.systemImage} onChange={e => handleSystemImageChange(e.target.value)}>
                {installedImages.map(id => (
                  <option key={id} value={id}>{getFriendlyName(id)}</option>
                ))}
              </select>
            )}
          </div>
          {form.systemImage.toLowerCase().includes('wear') ? (
            <div className="alert alert-info" style={{ fontSize: 12, marginBottom: 16 }}>
              ⌚ Smartwatch screens use native circular/square dimensions (390x390). Custom resolutions are locked for Wear OS stability.
            </div>
          ) : (
            <div className="form-group">
              <label className="form-label">Screen Resolution</label>
              <select className="form-select" value={form.screenResolution} onChange={e => set('screenResolution', e.target.value)}>
                {RESOLUTIONS.map(r => <option key={r.val} value={r.val}>{r.label}</option>)}
              </select>
            </div>
          )}
          <div className="form-group">
            <label className="form-label">GPU Rendering Mode</label>
            <select className="form-select" value={form.gpuMode} onChange={e => set('gpuMode', e.target.value)}>
              {GPU_MODES.map(g => <option key={g.val} value={g.val}>{g.label}</option>)}
            </select>
            <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 6 }}>
              {form.gpuMode === 'auto' && "💡 Recommended: Automatically select the best hardware renderer based on your system setup."}
              {form.gpuMode === 'host' && "💡 Force Hardware: Offload all 3D rendering tasks directly to your graphics card (dGPU/iGPU)."}
              {form.gpuMode === 'software' && "⚠️ Software Mode: Uses your CPU for graphics. Very slow, use only as a backup."}
            </div>
          </div>
          <div className="grid-2">
            <div className="form-group">
              <div className="flex items-center justify-between">
                <label className="form-label">RAM</label>
                <span className="text-accent font-mono" style={{ fontSize: 13 }}>{form.ram} MB</span>
              </div>
              <input type="range" min={1024} max={8192} step={512} value={form.ram} onChange={e => set('ram', Number(e.target.value))} />
              <div className="flex justify-between text-sm text-muted mt-2"><span>1 GB</span><span>8 GB</span></div>
            </div>
            <div className="form-group">
              <div className="flex items-center justify-between">
                <label className="form-label">CPU Cores</label>
                <span className="text-accent font-mono" style={{ fontSize: 13 }}>{form.cores} cores</span>
              </div>
              <input type="range" min={1} max={8} step={1} value={form.cores} onChange={e => set('cores', Number(e.target.value))} />
              <div className="flex justify-between text-sm text-muted mt-2"><span>1</span><span>8</span></div>
            </div>
          </div>
          <div className="grid-2">
            <div className="form-group">
              <div className="flex items-center justify-between">
                <label className="form-label">Internal Storage</label>
                <span className="text-accent font-mono" style={{ fontSize: 13 }}>{(form.storage / 1024).toFixed(0)} GB</span>
              </div>
              <input type="range" min={2048} max={32768} step={1024} value={form.storage} onChange={e => set('storage', Number(e.target.value))} />
            </div>
            <div className="form-group">
              <div className="flex items-center justify-between">
                <label className="form-label">Screen DPI</label>
                <span className="text-accent font-mono" style={{ fontSize: 13 }}>{form.dpi} dpi</span>
              </div>
              <input type="range" min={120} max={640} step={40} value={form.dpi} onChange={e => set('dpi', Number(e.target.value))} />
            </div>
          </div>
          <div className="alert alert-info" style={{ fontSize: 12 }}>
            💡 <strong>x86_64 images</strong> run natively on your PC. GPU mode <strong>host</strong> passes your GPU directly to the emulator for maximum performance.
          </div>

          <div className="divider" style={{ margin: '8px 0' }} />

          <div className="flex items-center justify-between" style={{ padding: '2px 0' }}>
            <div style={{ flex: 1, paddingRight: 16 }}>
              <div style={{ fontWeight: 600, fontSize: 12, marginBottom: 2 }}>🖥️ Show Qt Extended Sidebar</div>
              <div style={{ fontSize: 10, color: 'var(--text-muted)' }}>Show the Android Emulator extended controls panel (location, battery, etc). <span style={{ color: '#f87171' }}>May cause boot crash on some systems.</span></div>
            </div>
            <div className={`toggle ${form.showQtSidebar ? 'on' : ''}`} onClick={() => set('showQtSidebar', !form.showQtSidebar)} />
          </div>

          <div className="modal-footer">
            <button className="btn btn-ghost" onClick={onClose} disabled={creating}>Cancel</button>
            <button className="btn btn-primary" onClick={handleCreate} disabled={creating}>
              {creating ? <><Spinner size={14} />Creating...</> : '📱 Create Device'}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

export function DeviceCard({ avd, onLaunch, onStop, onDelete, onEdit, logs }) {
  const [launching, setLaunching] = useState(false)
  const [stopping, setStopping] = useState(false)
  const [showLogs, setShowLogs] = useState(false)
  const [copied, setCopied] = useState(false)
  const [showConfirmDelete, setShowConfirmDelete] = useState(false)

  const handleLaunch = async () => { setLaunching(true); await onLaunch(avd.name, false); setLaunching(false) }
  const handleWipeLaunch = async () => { setLaunching(true); await onLaunch(avd.name, true); setLaunching(false) }
  const handleStop = async () => { setStopping(true); await onStop(avd.name); setStopping(false) }
  
  const handleDelete = () => {
    setShowConfirmDelete(true)
  }

  const handleCopyLogs = () => {
    const logText = (logs[avd.name] || []).join('\n')
    navigator.clipboard.writeText(logText)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  const apiLevel = avd.api?.match(/android-(\d+)/)?.[1] || '?'
  const imgType = avd.api?.includes('playstore') ? '🎮 Play Store' : avd.api?.includes('google_apis') ? '🔬 Google APIs' : '🤖 AOSP'

  const getGpuBadge = () => {
    const mode = avd.gpu || 'auto';
    if (mode === 'host') {
      return (
        <span className="badge" style={{ background: 'rgba(239,68,68,0.12)', color: '#f87171', borderColor: 'rgba(239,68,68,0.25)', fontWeight: 600, fontSize: 10 }}>
          🔥 Hardware GPU
        </span>
      )
    }
    if (mode === 'software') {
      return (
        <span className="badge" style={{ background: 'rgba(156,163,175,0.12)', color: '#9ca3af', borderColor: 'rgba(156,163,175,0.25)', fontWeight: 500, fontSize: 10 }}>
          🖥️ Software GPU
        </span>
      )
    }
    return (
      <span className="badge" style={{ background: 'rgba(59,130,246,0.12)', color: '#60a5fa', borderColor: 'rgba(59,130,246,0.25)', fontWeight: 600, fontSize: 10 }}>
        ⚙️ Auto GPU
      </span>
    )
  }

  return (
    <div className={`device-card ${avd.running ? 'running' : ''}`}>
      <div className="flex items-center justify-between">
        <div>
          <div className="device-name">{avd.name.replace(/_/g, ' ')}</div>
          <div className="device-gpu-info">
            <span>🎮</span>
            <span>GPU: {avd.gpu || 'auto'} · {avd.cores || '4'} cores · {avd.ram ? Math.round(Number(avd.ram) / 1024) + 'GB' : '4GB'} RAM</span>
          </div>
        </div>
        <span className={`badge ${avd.running ? 'badge-running' : 'badge-stopped'}`}>
          {avd.running && <span className="dot" />}
          {avd.running ? 'Running' : 'Stopped'}
        </span>
      </div>
      <div className="device-meta">
        <span className="device-spec">API {apiLevel}</span>
        <span className="device-spec">{imgType}</span>
        <span className="device-spec">x86_64</span>
        {getGpuBadge()}
      </div>
      <div className="device-actions">
        {!avd.running ? (
          <>
            <button className="btn btn-success btn-sm" onClick={handleLaunch} disabled={launching}>
              {launching ? <><Spinner size={12} />Starting…</> : '▶ Launch'}
            </button>
            <button className="btn btn-ghost btn-sm" style={{ color: '#fbbf24' }} onClick={handleWipeLaunch} disabled={launching} title="Factory Reset & Boot">
              🧹 Wipe & Boot
            </button>
          </>
        ) : (
          <button className="btn btn-danger btn-sm" onClick={handleStop} disabled={stopping}>
            {stopping ? <><Spinner size={12} />Stopping…</> : '⏹ Stop'}
          </button>
        )}
        <button className="btn btn-ghost btn-sm" onClick={() => setShowLogs(v => !v)}>
          {showLogs ? '🙈 Hide Logs' : '📋 Logs'}
        </button>
        {(logs[avd.name] || []).length > 0 && (
          <button className="btn btn-ghost btn-sm" onClick={handleCopyLogs} style={{ color: copied ? 'var(--text-green)' : 'inherit' }}>
            {copied ? '✅ Copied!' : '🔗 Copy Logs'}
          </button>
        )}
        <div style={{ marginLeft: 'auto', display: 'flex', gap: '6px' }}>
          <button className="btn btn-ghost btn-sm btn-icon" onClick={() => onEdit(avd)}
            title="Edit Configuration" disabled={avd.running}>
            ✏️
          </button>
          <button className="btn btn-ghost btn-sm btn-icon" onClick={handleDelete}
            title="Delete device" style={{ color: '#ef4444' }} disabled={avd.running}>
            🗑️
          </button>
        </div>
      </div>
      {showLogs && <ConsoleLog lines={logs[avd.name] || []} />}

      {showConfirmDelete && (
        <div className="modal-overlay" onClick={() => setShowConfirmDelete(false)}>
          <div className="modal" style={{ width: 420 }} onClick={e => e.stopPropagation()}>
            <div className="modal-title" style={{ color: '#ef4444' }}>
              <span>⚠️</span> <span>Delete Device?</span>
            </div>
            <div style={{ padding: '16px 20px', fontSize: 13, lineHeight: '1.5', color: 'var(--text-secondary)' }}>
              Are you sure you want to delete <strong>{avd.name.replace(/_/g, ' ')}</strong>?
              <br /><br />
              This will permanently erase all data, apps, and storage associated with this emulator. 
              <strong style={{ color: '#ef4444' }}> This cannot be undone!</strong>
            </div>
            <div className="modal-footer" style={{ borderTop: '1px solid rgba(255,255,255,0.03)', marginTop: 0 }}>
              <button className="btn btn-ghost" onClick={() => setShowConfirmDelete(false)}>Cancel</button>
              <button className="btn btn-danger" onClick={async () => {
                setShowConfirmDelete(false)
                await onDelete(avd.name)
              }} style={{ background: '#ef4444', color: '#fff' }}>
                🗑️ Delete Permanently
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

export function EditAvdDialog({ avd, gpus, onClose, onSaved }) {
  const apiLower = (avd.api || '').toLowerCase()
  const targetLower = (avd.target || '').toLowerCase()
  const isWear = apiLower.includes('wear') || targetLower.includes('wear')
  const isTv = apiLower.includes('tv') || targetLower.includes('tv')
  const isAuto = apiLower.includes('automotive')

  const [ram, setRam] = useState(avd ? Number(avd.ram) || 4096 : 4096)
  const [cores, setCores] = useState(avd ? Number(avd.cores) || 4 : 4)
  const [gpuMode, setGpuMode] = useState(avd ? avd.gpu || 'auto' : 'auto')
  
  // Load per-device boot settings with fallbacks to global defaults
  const [quickBoot, setQuickBoot] = useState(() => {
    const val = localStorage.getItem(`emulator_quick_boot_${avd.name}`)
    if (val !== null) return val === 'true'
    return localStorage.getItem('emulator_quick_boot') === 'true'
  })
  const [bootAnim, setBootAnim] = useState(() => {
    const val = localStorage.getItem(`emulator_boot_anim_${avd.name}`)
    if (val !== null) return val !== 'false'
    return localStorage.getItem('emulator_boot_anim') !== 'false'
  })
  
  const [saving, setSaving] = useState(false)
  // Pre-fill resolution from avd config — fall back to the closest preset or '1280x720'
  const [resolution, setResolution] = useState(() => {
    if (avd.resolution) return avd.resolution
    if (avd.skin) {
      // skin is often stored as WxH e.g. "1080x1920"
      const parts = avd.skin.split('x')
      if (parts.length === 2 && !isNaN(parts[0]) && !isNaN(parts[1])) return avd.skin
    }
    return isWear ? '390x390' : isTv ? '1920x1080' : '1280x720'
  })
  const [dpi, setDpi] = useState(() => {
    if (avd.dpi) return Number(avd.dpi)
    return isWear ? 320 : isTv ? 240 : 240
  })
  const [ultraGaming, setUltraGaming] = useState(() => {
    return localStorage.getItem(`emulator_ultra_gaming_${avd.name}`) === 'true'
  })
  const [showQtSidebar, setShowQtSidebar] = useState(() => {
    return localStorage.getItem(`emulator_qt_sidebar_${avd.name}`) === 'true'
  })

  const handleSave = async () => {
    setSaving(true)
    const result = await api.updateAvdConfig({
      name: avd.name,
      cores: String(cores),
      ram: String(ram),
      gpu: gpuMode,
      resolution,
      dpi: String(dpi)
    })
    setSaving(false)
    if (result.ok) {
      // Save device-specific preferences
      localStorage.setItem(`emulator_quick_boot_${avd.name}`, String(quickBoot))
      localStorage.setItem(`emulator_boot_anim_${avd.name}`, String(bootAnim))
      localStorage.setItem(`emulator_ultra_gaming_${avd.name}`, String(ultraGaming))
      localStorage.setItem(`emulator_qt_sidebar_${avd.name}`, String(showQtSidebar))
      onSaved()
    } else {
      alert(result.error || 'Failed to update configuration')
    }
  }

  return (
    <div className="modal-overlay" onClick={e => e.target === e.currentTarget && onClose()}>
      <div className="modal" style={{ width: 500 }}>
        <div className="modal-title"><span>✏️</span><span>Edit AVD: {avd.name.replace(/_/g, ' ')}</span></div>
        <div className="modal-form">
          {isWear && (
            <div className="alert alert-warning" style={{ fontSize: 12, marginBottom: 16 }}>
              ⌚ <strong>Wear OS Watch detected!</strong> System resource limits have been automatically locked to watch specifications (Max 1GB RAM, Max 2 Cores, round screen layout preserved) to ensure a stable boot.
            </div>
          )}
          {isTv && (
            <div className="alert alert-warning" style={{ fontSize: 12, marginBottom: 16 }}>
              📺 <strong>Android TV detected!</strong> Resources clamped (Max 2GB RAM, Max 2 Cores) to maintain TV framework stability.
            </div>
          )}
          {isAuto && (
            <div className="alert alert-warning" style={{ fontSize: 12, marginBottom: 16 }}>
              🚗 <strong>Android Automotive detected!</strong> Resources clamped (Max 4GB RAM, Max 4 Cores) for optimal dashboard emulation.
            </div>
          )}
          <div className="form-group">
            <label className="form-label">GPU Rendering Mode</label>
            <select className="form-select" value={gpuMode} onChange={e => setGpuMode(e.target.value)}>
              {GPU_MODES.map(g => <option key={g.val} value={g.val}>{g.label}</option>)}
            </select>
            <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 6 }}>
              {gpuMode === 'auto' && "💡 Recommended: Automatically select the best hardware renderer based on your system setup."}
              {gpuMode === 'host' && "💡 Force Hardware: Offload all 3D rendering tasks directly to your graphics card (dGPU/iGPU)."}
              {gpuMode === 'software' && "⚠️ Software Mode: Uses your CPU for graphics. Very slow, use only as a backup."}
            </div>
          </div>

          <div className="form-group">
            <label className="form-label">Screen Resolution</label>
            <select className="form-select" value={resolution} onChange={e => setResolution(e.target.value)}>
              {RESOLUTIONS.map(r => <option key={r.val} value={r.val}>{r.label}</option>)}
            </select>
          </div>
          
          <div className="form-group">
            <div className="flex items-center justify-between">
              <label className="form-label">Screen Density (DPI)</label>
              <span className="text-accent font-mono" style={{ fontSize: 13 }}>{dpi} DPI</span>
            </div>
            <input type="range" min={160} max={480} step={40} value={dpi} onChange={e => setDpi(Number(e.target.value))} />
            <div className="flex justify-between text-sm text-muted mt-2"><span>160 (Low)</span><span>480 (High)</span></div>
          </div>
          
          <div className="form-group">
            <div className="flex items-center justify-between">
              <label className="form-label">RAM Allocation</label>
              <span className="text-accent font-mono" style={{ fontSize: 13 }}>{ram} MB</span>
            </div>
            <input type="range" min={1024} max={8192} step={512} value={ram} onChange={e => setRam(Number(e.target.value))} />
            <div className="flex justify-between text-sm text-muted mt-2"><span>1 GB</span><span>8 GB</span></div>
          </div>
          
          <div className="form-group">
            <div className="flex items-center justify-between">
              <label className="form-label">CPU Cores</label>
              <span className="text-accent font-mono" style={{ fontSize: 13 }}>{cores} cores</span>
            </div>
            <input type="range" min={1} max={8} step={1} value={cores} onChange={e => setCores(Number(e.target.value))} />
            <div className="flex justify-between text-sm text-muted mt-2"><span>1</span><span>8</span></div>
          </div>

          <div className="alert alert-info" style={{ fontSize: 12 }}>
            💡 <strong>Auto Mode</strong> is recommended. It automatically selects your dedicated card (NVIDIA RTX 2070 SUPER) and uses DirectX translation on integrated GPUs.
          </div>

          <div className="divider" style={{ margin: '8px 0' }} />

          <div className="flex items-center justify-between" style={{ padding: '2px 0' }}>
            <div style={{ flex: 1, paddingRight: 16 }}>
              <div style={{ fontWeight: 600, fontSize: 12, marginBottom: 2 }}>⚡ Quick Boot (Snapshot Resume)</div>
              <div style={{ fontSize: 10, color: 'var(--text-muted)' }}>Resume instantly from your last saved state (turn off for full cold boot).</div>
            </div>
            <div className={`toggle ${quickBoot ? 'on' : ''}`} onClick={() => setQuickBoot(!quickBoot)} />
          </div>

          <div className="divider" style={{ margin: '4px 0' }} />

          <div className="flex items-center justify-between" style={{ padding: '2px 0' }}>
            <div style={{ flex: 1, paddingRight: 16 }}>
              <div style={{ fontWeight: 600, fontSize: 12, marginBottom: 2 }}>🎬 Show Android Loading Logo</div>
              <div style={{ fontSize: 10, color: 'var(--text-muted)' }}>Display the native Android logo animation during startup.</div>
            </div>
            <div className={`toggle ${bootAnim ? 'on' : ''}`} onClick={() => setBootAnim(!bootAnim)} />
          </div>

          <div className="divider" style={{ margin: '4px 0' }} />

          <div className="flex items-center justify-between" style={{ padding: '2px 0' }}>
            <div style={{ flex: 1, paddingRight: 16 }}>
              <div style={{ fontWeight: 600, fontSize: 12, marginBottom: 2, color: 'var(--text-accent, #60a5fa)' }}>⚡ Ultra Gaming Mode (RAM Disk Cache)</div>
              <div style={{ fontSize: 10, color: 'var(--text-muted)' }}>Load disk writes directly into physical RAM. Instant loads and zero stutter. Note: Discards gameplay updates/cache on shutdown.</div>
            </div>
            <div className={`toggle ${ultraGaming ? 'on' : ''}`} onClick={() => setUltraGaming(!ultraGaming)} />
          </div>

          <div className="divider" style={{ margin: '4px 0' }} />

          <div className="flex items-center justify-between" style={{ padding: '2px 0' }}>
            <div style={{ flex: 1, paddingRight: 16 }}>
              <div style={{ fontWeight: 600, fontSize: 12, marginBottom: 2 }}>🖥️ Show Qt Extended Sidebar</div>
              <div style={{ fontSize: 10, color: 'var(--text-muted)' }}>Show the Android Emulator extended controls panel (location, battery, etc). <span style={{ color: '#f87171' }}>May cause boot crash on some systems.</span></div>
            </div>
            <div className={`toggle ${showQtSidebar ? 'on' : ''}`} onClick={() => setShowQtSidebar(!showQtSidebar)} />
          </div>
          
          <div className="modal-footer">
            <button className="btn btn-ghost" onClick={onClose} disabled={saving}>Cancel</button>
            <button className="btn btn-primary" onClick={handleSave} disabled={saving}>
              {saving ? <><Spinner size={14} />Saving...</> : '💾 Save Changes'}
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
