import { useState, useEffect } from 'react'
import { Spinner } from './UI.jsx'
import * as api from '../api.js'

export function GpuSettings({ toast, gpus, hypervisor, sysInfo, loading, onRescan, emojisEnabled = true }) {
  // Local emoji helper — respects the global "Emoji Everywhere" setting
  const emoji = (symbol) => emojisEnabled ? symbol : ''

  const [selectedGpu, setSelectedGpu] = useState(() => {
    if (!gpus || gpus.length === 0) return null;
    const dGpu = gpus.find(x => x.is_dedicated);
    return dGpu ? dGpu.index : gpus[0].index;
  });
  const [preference, setPreference] = useState(2)
  const [applying, setApplying] = useState(false)
  const [enablingWhpx, setEnablingWhpx] = useState(false)

  // Whether the Advanced Options panel is expanded
  const [advancedOpen, setAdvancedOpen] = useState(false)

  // Load custom emulator launch preferences from localStorage
  const [selectedAccel, setSelectedAccel] = useState(
    localStorage.getItem('emulator_accel') || 'whpx'
  )
  const [selectedGpuMode, setSelectedGpuMode] = useState(
    localStorage.getItem('emulator_gpu') || 'host'
  )
  const [quickBoot, setQuickBoot] = useState(
    localStorage.getItem('emulator_quick_boot') === 'true'
  )
  const [bootAnim, setBootAnim] = useState(
    localStorage.getItem('emulator_boot_anim') !== 'false'
  )

  // Performance Tweaks State
  const [noCamera, setNoCamera] = useState(
    localStorage.getItem('emulator_perf_no_camera') === 'true'
  )
  const [noGps, setNoGps] = useState(
    localStorage.getItem('emulator_perf_no_gps') === 'true'
  )
  const [noBluetooth, setNoBluetooth] = useState(
    localStorage.getItem('emulator_perf_no_bluetooth') === 'true'
  )

  const handlePerfToggle = (key, currentVal, setter, label) => {
    const val = !currentVal
    setter(val)
    localStorage.setItem(key, String(val))
    toast(val ? `${emoji('🔇')} ${label} disabled for speed` : `${emoji('🔊')} ${label} enabled`, 'success')
  }

  const handleQuickBootToggle = () => {
    const val = !quickBoot
    setQuickBoot(val)
    localStorage.setItem('emulator_quick_boot', String(val))
    toast(val ? `${emoji('⚡')} Quick Boot (Snapshots) enabled` : `${emoji('❄️')} Cold Boot mode enabled (starts fresh)`, 'success')
  }

  const handleBootAnimToggle = () => {
    const val = !bootAnim
    setBootAnim(val)
    localStorage.setItem('emulator_boot_anim', String(val))
    toast(val ? `${emoji('🎬')} Android Boot Animation enabled` : `${emoji('🚀')} Boot Animation disabled (faster startup)`, 'success')
  }

  useEffect(() => {
    if (selectedGpu === null && gpus && gpus.length > 0) {
      const dGpu = gpus.find(x => x.is_dedicated)
      setSelectedGpu(dGpu ? dGpu.index : gpus[0].index)
    }
  }, [gpus])

  const applyGpuPreference = async () => {
    if (selectedGpu === null) return
    setApplying(true)
    const result = await api.setGpuPreference({ gpuIndex: selectedGpu, gpuPreference: preference })
    setApplying(false)
    const ok = result?.ok ?? false
    toast(ok
      ? `${emoji('✅')} GPU preference applied! Emulator will use selected GPU on next launch.`
      : `${emoji('⚠️')} ${result?.error || 'Some registry entries may need administrator rights.'}`,
      ok ? 'success' : 'warn')
  }

  const enableWhpx = async () => {
    setEnablingWhpx(true)
    const result = await api.enableWhpx()
    setEnablingWhpx(false)
    toast(result.ok
      ? `${emoji('✅')} WHPX enabled — please restart your PC.`
      : `${emoji('❌')} Failed. Try running as administrator.`,
      result.ok ? 'success' : 'error')
  }

  const handleAccelChange = (val) => {
    setSelectedAccel(val)
    localStorage.setItem('emulator_accel', val)
    toast(`${emoji('⚡')} Launch Accelerator set to "${val.toUpperCase()}"`, 'success')
  }

  const handleGpuModeChange = (val) => {
    setSelectedGpuMode(val)
    localStorage.setItem('emulator_gpu', val)
    toast(`${emoji('🎮')} Default launch GPU set to "${val.toUpperCase()}"`, 'success')
  }

  if (loading || !sysInfo) return (
    <div className="flex items-center justify-center" style={{ padding: 60, gap: 14 }}>
      <Spinner size={24} />
      <span className="text-muted">Scanning hardware configurations...</span>
    </div>
  )

  const GPU_PREFS = [
    { val: 1, label: `${emoji('🔋')} Power Saving (iGPU)`, desc: 'Use integrated graphics — saves battery' },
    { val: 2, label: `${emoji('🔥')} High Performance (dGPU)`, desc: 'Use dedicated GPU — maximum gaming performance' },
    { val: 0, label: `${emoji('⚙️')} System Default`, desc: 'Let Windows decide which GPU to use' },
  ]

  const ACCEL_OPTIONS = [
    { val: 'whpx', label: `${emoji('⚡')} WHPX (Windows Hypervisor Platform) - Recommended`, desc: 'Best performance. Fully compatible with WSL2, Hyper-V, Docker, and VirtualBox.' },
    { val: 'aehd', label: `${emoji('🛡️')} AEHD (Android Emulator Hypervisor Driver)`, desc: "Google's lightweight standalone driver. Works on Intel/AMD. (Must disable Hyper-V to run AEHD)." },
    { val: 'haxm', label: `${emoji('⚙️')} Intel HAXM (Legacy)`, desc: "Intel's hardware acceleration driver. Works on Intel CPUs only (Deprecated)." },
    { val: 'off',  label: `${emoji('🖥️')} Off (No Virtualization)`, desc: 'Extremely slow! Emulates CPU in software. Use only for diagnostic troubleshooting.' },
  ]

  const GPU_MODES = [
    { val: 'host', label: `${emoji('🔥')} Host GPU (Zero overhead pass-through)`, desc: 'Direct access to your selected graphics card. Best for 3D gaming.' },
    { val: 'angle_indirect', label: `${emoji('⚡')} ANGLE (DirectX translation)`, desc: 'Translates OpenGL calls into Windows DirectX. Highly stable.' },
    { val: 'swiftshader_indirect', label: `${emoji('🖥️')} SwiftShader (Software)`, desc: 'CPU-rendered graphics. Slow, compatibility mode only.' },
  ]

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 24 }}>
      <div className="flex items-center justify-between" style={{ paddingBottom: 4 }}>
        <div style={{ fontSize: 13, color: 'var(--text-secondary)' }}>
          Hardware configuration is cached. Use rescan if you made hardware changes.
        </div>
        <button className="btn btn-ghost btn-sm" onClick={onRescan} disabled={loading} style={{ border: '1px solid var(--border)' }}>
          {emoji('🔄')} Rescan Hardware
        </button>
      </div>

      {/* ── Hardware Summary Cards ── */}
      {sysInfo && (
        <div className="grid-3">
          {[
            { icon: emoji('💻') || '🖥', label: 'CPU', val: sysInfo.cpu_model?.split('@')[0]?.trim(), sub: `${sysInfo.cpu_count} threads` },
            { icon: emoji('🧠') || '💾', label: 'Total RAM', val: `${Math.round(sysInfo.total_ram / 1024)} GB`, sub: `${Math.round(sysInfo.free_ram / 1024)} GB free` },
            { icon: emoji('🖥️') || '📟', label: 'Architecture', val: sysInfo.arch, sub: sysInfo.platform },
          ].map(s => (
            <div key={s.label} className="stat-card">
              <div className="stat-icon" style={{ background: 'rgba(120,80,255,0.15)' }}>{s.icon}</div>
              <div>
                <div className="stat-label">{s.label}</div>
                <div className="stat-value" style={{ fontSize: 14 }}>{s.val}</div>
                <div className="stat-sub">{s.sub}</div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* ── Virtualization Accelerator ── */}
      <div className="card">
        <div className="card-title">{emoji('⚡')} Virtualization Accelerator (accel)</div>
        <div style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 16 }}>
          Choose which CPU virtualization driver to use when launching emulators.
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          {ACCEL_OPTIONS.map(opt => (
            <label key={opt.val} className={`gpu-card ${selectedAccel === opt.val ? 'selected' : ''}`} style={{ cursor: 'pointer' }} onClick={() => handleAccelChange(opt.val)}>
              <input type="radio" name="accel" checked={selectedAccel === opt.val} onChange={() => handleAccelChange(opt.val)} style={{ accentColor: '#7c3aed' }} />
              <div>
                <div className="gpu-name">{opt.label}</div>
                <div className="gpu-vram">{opt.desc}</div>
              </div>
            </label>
          ))}
        </div>

        {selectedAccel === 'whpx' && !hypervisor?.whpx_enabled && (
          <div className="alert alert-warn mt-3" style={{ display: 'flex', justifyContent: 'between', alignItems: 'center' }}>
            <span>{emoji('⚠️')} WHPX is selected but not enabled in Windows.</span>
            <button className="btn btn-primary btn-sm" onClick={enableWhpx} disabled={enablingWhpx}>
              {enablingWhpx ? 'Enabling...' : 'Enable WHPX Now'}
            </button>
          </div>
        )}
      </div>

      {/* ── GPU Launch Rendering Mode ── */}
      <div className="card">
        <div className="card-title">{emoji('🎮')} Launch GPU Rendering Mode (gpu)</div>
        <div style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 16 }}>
          Select the graphics acceleration mode passed to the emulator on startup.
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          {GPU_MODES.map(opt => (
            <label key={opt.val} className={`gpu-card ${selectedGpuMode === opt.val ? 'selected' : ''}`} style={{ cursor: 'pointer' }} onClick={() => handleGpuModeChange(opt.val)}>
              <input type="radio" name="gpu_mode" checked={selectedGpuMode === opt.val} onChange={() => handleGpuModeChange(opt.val)} style={{ accentColor: '#7c3aed' }} />
              <div>
                <div className="gpu-name">{opt.label}</div>
                <div className="gpu-vram">{opt.desc}</div>
              </div>
            </label>
          ))}
        </div>
      </div>

      {/* ── Launch & Boot Options ── */}
      <div className="card">
        <div className="card-title">{emoji('🚀')} Launch &amp; Boot Options</div>
        <div style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 16 }}>
          Configure emulator startup and loading screen behaviors.
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
          <div className="flex items-center justify-between" style={{ padding: '4px 0' }}>
            <div style={{ flex: 1, paddingRight: 16 }}>
              <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 2 }}>{emoji('⚡')} Quick Boot (Snapshot Resume)</div>
              <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>Resume instantly from your last saved state (turn off for full cold boot every time).</div>
            </div>
            <div className={`toggle ${quickBoot ? 'on' : ''}`} onClick={handleQuickBootToggle} />
          </div>

          <div className="divider" style={{ margin: '4px 0' }} />

          <div className="flex items-center justify-between" style={{ padding: '4px 0' }}>
            <div style={{ flex: 1, paddingRight: 16 }}>
              <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 2 }}>{emoji('🎬')} Show Android Loading Logo</div>
              <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>Display the native Android logo animation during startup.</div>
            </div>
            <div className={`toggle ${bootAnim ? 'on' : ''}`} onClick={handleBootAnimToggle} />
          </div>
        </div>
      </div>

      {/* ── Advanced Options (collapsible) ── */}
      <div className="card" style={{ padding: 0, overflow: 'hidden' }}>
        {/* Header / toggle */}
        <button
          id="btn-advanced-options-toggle"
          onClick={() => setAdvancedOpen(o => !o)}
          style={{
            width: '100%',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: '16px 20px',
            background: 'none',
            border: 'none',
            cursor: 'pointer',
            color: 'var(--text-primary)',
            fontWeight: 600,
            fontSize: 14,
            textAlign: 'left',
          }}
        >
          <span>{emoji('🔧')} Advanced Options</span>
          <span style={{
            fontSize: 11,
            color: 'var(--text-muted)',
            background: 'rgba(120,80,255,0.12)',
            padding: '3px 10px',
            borderRadius: 20,
            display: 'flex',
            alignItems: 'center',
            gap: 6,
          }}>
            {advancedOpen ? `${emoji('▲')} Collapse` : `${emoji('▼')} Expand`}
          </span>
        </button>

        {advancedOpen && (
          <div style={{ padding: '0 20px 20px', display: 'flex', flexDirection: 'column', gap: 24 }}>
            {/* Speed & Performance Tweaks */}
            <div>
              <div className="card-title" style={{ marginBottom: 8 }}>{emoji('⚡')} Speed &amp; Performance Tweaks</div>
              <div style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 16 }}>
                Disable unused emulator hardware modules to speed up launch times and reduce host CPU/RAM usage.
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
                <div className="flex items-center justify-between" style={{ padding: '4px 0' }}>
                  <div style={{ flex: 1, paddingRight: 16 }}>
                    <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 2 }}>{emoji('📷')} Disable Webcam &amp; Camera Emulation</div>
                    <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>Disables front/back virtual cameras. Bypasses 3D scene rendering for faster startup.</div>
                  </div>
                  <div className={`toggle ${noCamera ? 'on' : ''}`} onClick={() => handlePerfToggle('emulator_perf_no_camera', noCamera, setNoCamera, 'Camera Emulation')} />
                </div>

                <div className="divider" style={{ margin: '4px 0' }} />

                <div className="flex items-center justify-between" style={{ padding: '4px 0' }}>
                  <div style={{ flex: 1, paddingRight: 16 }}>
                    <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 2 }}>{emoji('📍')} Disable Location / GPS Polling</div>
                    <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>Stops the emulator from continuously tracking your computer's location.</div>
                  </div>
                  <div className={`toggle ${noGps ? 'on' : ''}`} onClick={() => handlePerfToggle('emulator_perf_no_gps', noGps, setNoGps, 'GPS Tracking')} />
                </div>

                <div className="divider" style={{ margin: '4px 0' }} />

                <div className="flex items-center justify-between" style={{ padding: '4px 0' }}>
                  <div style={{ flex: 1, paddingRight: 16 }}>
                    <div style={{ fontWeight: 600, fontSize: 13, marginBottom: 2 }}>{emoji('📡')} Disable Bluetooth Simulation</div>
                    <div style={{ fontSize: 11, color: 'var(--text-muted)' }}>Disables the background Bluetooth network daemon (netsimd), saving ~100MB of RAM.</div>
                  </div>
                  <div className={`toggle ${noBluetooth ? 'on' : ''}`} onClick={() => handlePerfToggle('emulator_perf_no_bluetooth', noBluetooth, setNoBluetooth, 'Bluetooth Simulation')} />
                </div>
              </div>
            </div>

            <div className="divider" />

            {/* Windows GPU Preference (Registry) */}
            <div>
              <div className="card-title" style={{ marginBottom: 8 }}>{emoji('🔌')} Windows GPU Preference (Registry Override)</div>
              <div style={{ fontSize: 13, color: 'var(--text-secondary)', marginBottom: 16 }}>
                Force Windows Graphics settings to bind the emulator executables directly to your dedicated or integrated GPU.
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 10, marginBottom: 20 }}>
                {gpus.length === 0 && <div className="alert alert-warn">No GPUs detected. Check your GPU drivers.</div>}
                {gpus.map(gpu => (
                  <div key={gpu.index} className={`gpu-card ${selectedGpu === gpu.index ? 'selected' : ''}`}
                    onClick={() => setSelectedGpu(gpu.index)}>
                    <div className={`gpu-icon ${gpu.is_dedicated ? 'dedicated' : 'integrated'}`}>
                      {gpu.is_dedicated ? emoji('🔥') || '★' : emoji('💡') || '○'}
                    </div>
                    <div style={{ flex: 1 }}>
                      <div className="gpu-name">{gpu.name}</div>
                      <div className="gpu-vram">VRAM: {gpu.vram} · {gpu.is_dedicated ? 'Dedicated GPU (High Performance)' : 'Integrated GPU (Power Saving)'}</div>
                    </div>
                    {gpu.is_dedicated && <span className="badge badge-gpu">Recommended</span>}
                    {selectedGpu === gpu.index && <span style={{ color: '#7c3aed', fontSize: 18 }}>✓</span>}
                  </div>
                ))}
              </div>

              <div className="form-group" style={{ marginBottom: 20 }}>
                <label className="form-label">Windows Registry GPU Preference Mode</label>
                <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                  {GPU_PREFS.map(p => (
                    <label key={p.val} className="gpu-card" style={{ cursor: 'pointer' }} onClick={() => setPreference(p.val)}>
                      <input type="radio" name="win_gpu_pref" checked={preference === p.val} onChange={() => setPreference(p.val)} style={{ accentColor: '#7c3aed' }} />
                      <div>
                        <div className="gpu-name">{p.label}</div>
                        <div className="gpu-vram">{p.desc}</div>
                      </div>
                    </label>
                  ))}
                </div>
              </div>

              <div className="alert alert-info" style={{ marginBottom: 16 }}>
                {emoji('💡')} Written to <span className="font-mono" style={{ fontSize: 11 }}>HKCU\Software\Microsoft\DirectX\UserGpuPreferences</span> — same as Windows <strong>Settings → Display → Graphics Settings</strong>.
              </div>

              <button className="btn btn-primary" onClick={applyGpuPreference} disabled={applying || selectedGpu === null}>
                {applying ? <><Spinner size={15} />Applying…</> : `${emoji('🎮')} Apply Windows GPU Registry`}
              </button>
            </div>

            <div className="divider" />

            {/* Performance Tips */}
            <div>
              <div className="card-title" style={{ marginBottom: 12 }}>{emoji('🚀')} Performance Tips</div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
                {[
                  { icon: emoji('🎮') || '★', tip: 'Use GPU mode "Host" when creating a device — passes graphics directly to your GPU with zero translation overhead.' },
                  { icon: emoji('📦') || '□', tip: 'Always use x86_64 system images — no ARM emulation overhead, runs at native CPU speed.' },
                  { icon: emoji('🧠') || '○', tip: 'Allocate at least 4 CPU cores and 4 GB RAM for gaming apps.' },
                  { icon: emoji('⚡') || '!', tip: 'Enable WHPX (Windows Hypervisor Platform) for CPU virtualization — massive speed improvement.' },
                  { icon: emoji('💾') || '◈', tip: 'Use fast boot (snapshot) to avoid waiting for Android to cold boot each time.' },
                  { icon: emoji('🔋') || '▷', tip: 'Close other GPU-heavy apps while gaming in the emulator for best frame rates.' },
                ].map((t, i) => (
                  <div key={i} className="flex gap-2 items-center" style={{ padding: '10px 0', borderBottom: i < 5 ? '1px solid var(--border)' : 'none' }}>
                    <span style={{ fontSize: 18, width: 28 }}>{t.icon}</span>
                    <span style={{ fontSize: 13, color: 'var(--text-secondary)', lineHeight: 1.5 }}>{t.tip}</span>
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
