import { useState, useEffect } from 'react'
import {
  AlertTriangle,
  CarFront,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Cpu,
  Download,
  HardDrive,
  PackageOpen,
  RefreshCw,
  Search,
  ShieldCheck,
  Smartphone,
  Sparkles,
  Trash2,
  Tv,
  Watch,
} from 'lucide-react'
import { Spinner, useConfirm } from './UI.jsx'
import * as api from '../api.js'

const CORE_TOOLS = [
  { id: 'emulator', name: 'Android Emulator', desc: 'Core emulator engine (required)', icon: <Cpu size={14} />, required: true },
  { id: 'platform-tools', name: 'Platform Tools (ADB/Fastboot)', desc: 'ADB, fastboot, and platform utilities', icon: <ShieldCheck size={14} />, required: true }
]

const TABS = [
  { id: 'stable_playstore', label: 'Stable (Play Store)', icon: <Smartphone size={12} /> },
  { id: 'stable_google', label: 'Stable (Google APIs / AOSP)', icon: <Sparkles size={12} /> },
  { id: 'beta', label: 'Beta / Previews', icon: <Cpu size={12} /> },
  { id: 'tv', label: 'Android TV', icon: <Tv size={12} /> },
  { id: 'wear', label: 'Wear OS', icon: <Watch size={12} /> },
  { id: 'automotive', label: 'Automotive', icon: <CarFront size={12} /> },
]

// Helper to parse package metadata dynamically and future-proof it
function parseImageDetails(img) {
  const id = String(img?.id || '');
  const name = String(img?.name || '');

  // Example ID: system-images;android-35;google_apis_playstore;x86_64
  const parts = id.split(';');

  let apiLevel = 0;
  let rawApi = "";
  if (parts.length >= 2) {
    rawApi = parts[1].replace('android-', '');
    apiLevel = parseInt(rawApi) || 0;
  }

  if (apiLevel === 0) {
    const apiMatch = name.match(/API\s+(\d+)/i) || id.match(/android-(\d+)/i);
    if (apiMatch) {
      apiLevel = parseInt(apiMatch[1]) || 0;
    }
  }

  // Determine Android OS Version
  let androidVer = "";
  if (apiLevel >= 33) {
    androidVer = String(apiLevel - 20); // API 33 -> 13, API 34 -> 14, API 35 -> 15...
  } else if (apiLevel === 31 || apiLevel === 32) {
    androidVer = "12";
  } else if (apiLevel === 30) {
    androidVer = "11";
  } else if (apiLevel > 0) {
    androidVer = String(apiLevel - 20 > 0 ? apiLevel - 20 : apiLevel);
  } else {
    androidVer = "Preview";
  }

  const isWear = id.includes('wear') || name.toLowerCase().includes('wear');
  const isTv = id.includes('tv') || id.includes('google-tv') || name.toLowerCase().includes('tv');
  const isAuto = id.includes('automotive') || name.toLowerCase().includes('auto');
  let deviceType = "Phone / Tablet";
  let typeLabel = "Phone";
  if (isWear) {
    deviceType = "Wear OS Watch";
    typeLabel = "Wear OS";
  } else if (isTv) {
    deviceType = "Android TV";
    typeLabel = "Android TV";
  } else if (isAuto) {
    deviceType = "Automotive Car";
    typeLabel = "Automotive";
  }

  // System services/APIs
  let services = "Google APIs";
  let shortServices = "Google APIs";
  if (id.includes('playstore') || name.toLowerCase().includes('playstore') || name.toLowerCase().includes('play store')) {
    services = "Google Play Store (with Services)";
    shortServices = "Play Store";
  } else if (id.includes('google-tv') || id.includes('google_tv') || name.toLowerCase().includes('google tv') || name.toLowerCase().includes('google-tv')) {
    services = "Google TV (with Play Services)";
    shortServices = "Google TV";
  } else if (id.includes('android-tv') || id.includes('android_tv')) {
    services = "Android TV (with Services)";
    shortServices = "Android TV";
  } else if (id.includes('google_apis') || name.toLowerCase().includes('google apis')) {
    services = "Google APIs Developer Image";
    shortServices = "Google APIs";
  } else {
    services = "AOSP Pure Android";
    shortServices = "AOSP";
  }

  const is16k = id.includes('ps16k') || id.includes('16k') || id.includes('16kb');
  // API levels indicating Beta/Preview are identified by tags like 'preview' or 'beta'
  const isPreview = id.includes('preview') || id.includes('beta') || name.toLowerCase().includes('preview') || name.toLowerCase().includes('beta') || is16k;
  const stability = isPreview ? "Beta / Preview" : "Stable";

  return {
    apiLevel,
    androidVer,
    typeLabel,
    services,
    shortServices,
    isWear,
    isTv,
    isAuto,
    is16k,
    isPreview,
    stability
  };
}

export function SdkManager({
  status,
  refreshStatus,
  progress,
  setProgress,
  installing,
  setInstalling,
  errors,
  setErrors
}) {
  const [systemImages, setSystemImages] = useState([])
  const [loadingImages, setLoadingImages] = useState(false)
  const [search, setSearch] = useState('')
  const [activeTab, setActiveTab] = useState('stable_playstore')
  const [showCoreDetails, setShowCoreDetails] = useState(false)
  const [expandedGroups, setExpandedGroups] = useState({})
  const { confirm, dialog: confirmDialog } = useConfirm()

  // Derive variables from status & systemImages
  const installedList = status?.installed_packages || []
  const emulatorInstalled = status?.emulator_installed;
  const platformToolsInstalled = status?.platform_tools_installed;
  const coreToolsReady = emulatorInstalled && platformToolsInstalled;

  const validSystemImages = systemImages.filter(img => img && img.id && img.name);

  // Filter dynamic images based on active tab and search input
  const filteredImages = validSystemImages.filter(img => {
    const details = parseImageDetails(img)
    const nameStr = String(img?.name || '');
    const idStr = String(img?.id || '');

    const matchesSearch = nameStr.toLowerCase().includes(search.toLowerCase()) ||
      idStr.toLowerCase().includes(search.toLowerCase());
    if (!matchesSearch) return false

    const isWear = idStr.includes('wear') || nameStr.toLowerCase().includes('wear');
    const isTv = idStr.includes('tv') || idStr.includes('google-tv') || nameStr.toLowerCase().includes('tv');
    const isAuto = idStr.includes('automotive') || nameStr.toLowerCase().includes('automotive');

    switch (activeTab) {
      case 'stable_playstore':
        return details.stability === 'Stable' && details.shortServices === 'Play Store' && !isWear && !isTv && !isAuto;
      case 'stable_google':
        return details.stability === 'Stable' && details.shortServices !== 'Play Store' && !isWear && !isTv && !isAuto;
      case 'beta':
        return details.stability === 'Beta / Preview' && !isWear && !isTv && !isAuto;
      case 'tv':
        return isTv;
      case 'wear':
        return isWear;
      case 'automotive':
        return isAuto;
      default:
        return true;
    }
  })

  // Deduplicate: If there are multiple packages with the same variant and base API level, keep only the latest version.
  const dedupedImages = [];
  const seen = {};
  const sortedImagesForDedup = [...filteredImages].sort((a, b) => {
    const detailsA = parseImageDetails(a)
    const detailsB = parseImageDetails(b)
    return detailsB.apiLevel - detailsA.apiLevel; // higher API level first
  });

  sortedImagesForDedup.forEach(img => {
    const idParts = img.id.split(';');
    if (idParts.length >= 4) {
      const rawVer = idParts[1].replace('android-', '');
      const baseVer = rawVer.split('.')[0]; // e.g. "35"
      const variant = idParts[2];
      const key = `${baseVer}-${variant}`;
      if (!seen[key]) {
        seen[key] = true;
        dedupedImages.push(img);
      }
    } else {
      dedupedImages.push(img);
    }
  });

  // Group dedupedImages by API level dynamically (future-proof)
  const groups = {};
  dedupedImages.forEach(img => {
    const details = parseImageDetails(img);
    const key = String(details.apiLevel);
    if (!groups[key]) {
      groups[key] = [];
    }
    groups[key].push(img);
  });

  // Sort groups descending (e.g. API 37 before API 36)
  const sortedGroupKeys = Object.keys(groups).sort((a, b) => Number(b) - Number(a));

  // No local progress subscription needed, as progress is now lifted to App.jsx

  const loadSdkImages = async () => {
    setLoadingImages(true)
    try {
      const list = await api.fetchSdkPackages()
      setSystemImages(Array.isArray(list) ? list : [])
    } catch (e) {
      console.error(e)
    }
    setLoadingImages(false)
  }

  useEffect(() => {
    if (status?.cmdline_installed && status?.jdk_installed) {
      loadSdkImages()
    }
  }, [status?.cmdline_installed, status?.jdk_installed])

  const installPkg = async (pkgId) => {
    setInstalling(s => ({ ...s, [pkgId]: true }))
    setErrors(s => ({ ...s, [pkgId]: null }))
    try {
      const result = await api.installPackage({ packageId: pkgId })
      if (!result.ok) {
        setErrors(s => ({ ...s, [pkgId]: result.error || 'Installation failed' }))
      } else {
        if (refreshStatus) await refreshStatus()
        await loadSdkImages()
      }
    } catch (e) {
      setErrors(s => ({ ...s, [pkgId]: e.toString() }))
    }
    setInstalling(s => ({ ...s, [pkgId]: false }))
  }

  const uninstallPkg = async (pkgId) => {
    if (!await confirm("Are you sure you want to delete this system image?\n\nThis will remove the system files from your disk. Any created devices using this image won't be able to boot until reinstalled.")) return
    setInstalling(s => ({ ...s, [pkgId]: true }))
    setErrors(s => ({ ...s, [pkgId]: null }))
    try {
      const result = await api.uninstallPackage({ packageId: pkgId })
      if (!result.ok) {
        setErrors(s => ({ ...s, [pkgId]: result.error || 'Deletion failed' }))
      } else {
        if (refreshStatus) await refreshStatus()
        await loadSdkImages()
      }
    } catch (e) {
      setErrors(s => ({ ...s, [pkgId]: e.toString() }))
    }
    setInstalling(s => ({ ...s, [pkgId]: false }))
  }

  const acceptLicenses = async () => {
    setInstalling(s => ({ ...s, _licenses: true }))
    try {
      await api.acceptLicenses()
    } catch (e) {
      setErrors(s => ({ ...s, _licenses: e.toString() }))
    }
    setInstalling(s => ({ ...s, _licenses: false }))
  }

  // Auto-expand the latest (first) group by default when tab/list changes
  useEffect(() => {
    if (sortedGroupKeys.length > 0) {
      const latest = sortedGroupKeys[0];
      setExpandedGroups(s => {
        if (s[latest] === undefined) {
          return { [latest]: true };
        }
        return s;
      });
    }
  }, [activeTab, systemImages, search]);

  // Block if cmdline tools not ready (Render this check AFTER all hook definitions)
  if (!status?.cmdline_installed || !status?.jdk_installed) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
        <div className="alert alert-warn">
          <AlertTriangle size={14} style={{ marginRight: 6 }} /> <strong>Setup not complete!</strong> You must install <strong>JDK</strong> and <strong>cmdline-tools</strong> first before using the SDK Manager.
          Go to the <strong>Setup / Install</strong> page in the sidebar and complete all steps.
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          {[
            { label: 'Step 1: Portable OpenJDK 21', done: status?.jdk_installed },
            { label: 'Step 2: Android cmdline-tools', done: status?.cmdline_installed },
            { label: 'Step 3: Use SDK Manager below', done: false },
          ].map((s, i) => (
            <div key={i} className="pkg-row">
              <span style={{ fontSize: 20, display: 'inline-flex', alignItems: 'center' }}>{s.done ? <CheckCircle2 size={18} /> : <RefreshCw size={18} style={{ opacity: 0.7 }} />}</span>
              <span className="pkg-name" style={{ color: s.done ? 'var(--text-green)' : 'var(--text-muted)' }}>{s.label}</span>
            </div>
          ))}
        </div>
      </div>
    )
  }

  return (
    <>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>

        {/* ─── Part 1: Core Required Emulator Tools (Compact Status Header) ─── */}
        <div className="section" style={{ marginBottom: 8 }}>
          {coreToolsReady ? (
            <div className="alert" style={{
              background: 'rgba(16, 185, 129, 0.04)',
              borderColor: 'rgba(16, 185, 129, 0.2)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              padding: '10px 14px',
              borderRadius: '6px'
            }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12 }}>
                <CheckCircle2 size={14} style={{ color: '#34d399' }} />
                <span><strong>Emulator Core Engine</strong> is fully operational and ready.</span>
              </div>
              <button
                className="btn btn-ghost btn-sm"
                style={{ padding: '3px 8px', fontSize: 10 }}
                onClick={() => setShowCoreDetails(!showCoreDetails)}
              >
                {showCoreDetails ? 'Hide Details' : 'Show Core Tools'}
              </button>
            </div>
          ) : (
            <div className="alert alert-warn" style={{
              display: 'flex',
              flexDirection: 'column',
              gap: 10,
              padding: '12px 14px',
              borderRadius: '6px'
            }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12 }}>
                <AlertTriangle size={14} style={{ color: '#fbbf24' }} />
                <span><strong>Core Emulator Tools are missing!</strong> You must download them to run or create virtual devices.</span>
              </div>
              <div style={{ display: 'flex', gap: 8 }}>
                {CORE_TOOLS.map(pkg => {
                  const installed = installedList.includes(pkg.id) ||
                    (pkg.id === 'emulator' && emulatorInstalled) ||
                    (pkg.id === 'platform-tools' && platformToolsInstalled);
                  if (installed) return null;
                  const isInstalling = installing[pkg.id];
                  return (
                    <button key={pkg.id} className="btn btn-sm btn-primary" onClick={() => installPkg(pkg.id)} disabled={isInstalling}>
                      {isInstalling ? <Spinner size={10} /> : <><Download size={10} style={{ marginRight: 4 }} /> Download</>} {pkg.name}
                    </button>
                  );
                })}
              </div>
            </div>
          )}

          {/* Expandable Core Tools list */}
          {showCoreDetails && (
            <div style={{
              marginTop: 10,
              display: 'flex',
              flexDirection: 'column',
              gap: 8,
              padding: 10,
              background: 'rgba(255,255,255,0.01)',
              border: '1px dashed var(--border)',
              borderRadius: 6
            }}>
              {CORE_TOOLS.map(pkg => {
                const isInstalled = installedList.includes(pkg.id) ||
                  (pkg.id === 'emulator' && emulatorInstalled) ||
                  (pkg.id === 'platform-tools' && platformToolsInstalled);
                const isInstalling = installing[pkg.id];
                const pct = progress[pkg.id] || 0;
                return (
                  <div key={pkg.id} style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                    <div className="pkg-row" style={{ padding: '6px 10px' }}>
                      <span style={{ fontSize: 16 }}>{pkg.icon}</span>
                      <div style={{ flex: 1 }}>
                        <div className="pkg-name" style={{ fontSize: 12 }}>
                          {pkg.name}
                          {isInstalled && <span className="badge badge-ok" style={{ marginLeft: 8 }}>Installed</span>}
                        </div>
                      </div>
                      {!isInstalled && (
                        <button className="btn btn-sm btn-primary" onClick={() => installPkg(pkg.id)} disabled={!!isInstalling}>
                          {isInstalling ? <Spinner size={10} /> : <><Download size={10} style={{ marginRight: 4 }} /> Install</>}
                        </button>
                      )}
                    </div>
                    {isInstalling && (
                      <div style={{ width: '100%', height: 3, background: 'rgba(255,255,255,0.05)', overflow: 'hidden' }}>
                        <div style={{ width: `${pct}%`, height: '100%', background: 'var(--text-accent)' }} />
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* ─── Part 1.5: Installed System Images (With Delete Capability) ─── */}
        {(() => {
          const installedImages = validSystemImages.filter(img => img.installed || installedList.includes(img.id));
          if (installedImages.length === 0) return null;
          return (
            <div className="section">
              <div className="section-title"><HardDrive size={14} style={{ marginRight: 6 }} /> Downloaded OS Images ({installedImages.length})</div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                {installedImages.map(img => {
                  const isInstalling = installing[img.id];
                  const pct = progress[img.id] || 0;
                  const details = parseImageDetails(img);

                  return (
                    <div key={img.id} style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                      <div className="pkg-row" style={{
                        background: 'rgba(255,255,255,0.01)',
                        borderColor: 'rgba(255,255,255,0.06)',
                        padding: '8px 12px'
                      }}>
                        <span style={{ fontSize: 18, display: 'inline-flex', alignItems: 'center', justifyContent: 'center' }}>
                          {details.isWear ? <Watch size={16} /> : details.isTv ? <Tv size={16} /> : details.isAuto ? <CarFront size={16} /> : <Smartphone size={16} />}
                        </span>
                        <div style={{ flex: 1 }}>
                          <div className="pkg-name" style={{ fontSize: 12, fontWeight: 600 }}>
                            Android {details.androidVer} ({details.typeLabel})
                            <span className={`badge ${details.isPreview ? 'badge-warn' : 'badge-ok'}`} style={{ marginLeft: 8, fontSize: 8 }}>
                              {details.stability}
                            </span>
                            {details.is16k && <span className="badge badge-error" style={{ marginLeft: 4, fontSize: 8 }}>16 KB</span>}
                          </div>
                          <div className="pkg-desc" style={{ fontSize: 10, color: 'var(--text-secondary)' }}>
                            {details.services} · <span className="font-mono">API {details.apiLevel}</span>
                          </div>
                        </div>
                        <div style={{ display: 'flex', gap: 6 }}>
                          <button className="btn btn-sm btn-ghost" style={{ padding: '4px 8px', fontSize: 10 }}
                            onClick={() => installPkg(img.id)} disabled={!!isInstalling}>
                            {isInstalling ? <Spinner size={10} /> : <><RefreshCw size={10} style={{ marginRight: 4 }} /> Repair</>}
                          </button>
                          <button className="btn btn-sm btn-ghost" style={{
                            padding: '4px 8px',
                            fontSize: 10,
                            color: '#f87171',
                            borderColor: 'rgba(239, 68, 68, 0.15)'
                          }}
                            onClick={() => uninstallPkg(img.id)} disabled={!!isInstalling}>
                            <Trash2 size={10} style={{ marginRight: 4 }} /> Delete
                          </button>
                        </div>
                      </div>
                      {isInstalling && (
                        <div style={{ display: 'flex', flexDirection: 'column', gap: 2, width: '100%', padding: '0 8px 6px 8px' }}>
                          <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 9, color: 'var(--text-accent)' }}>
                            <span>Modifying system files...</span>
                            <span className="font-mono">{pct}%</span>
                          </div>
                          <div className="progress-wrap" style={{ height: 4 }}>
                            <div className="progress-bar" style={{ width: `${pct}%`, background: 'var(--text-accent)' }} />
                          </div>
                        </div>
                      )}
                      {errors[img.id] && <div className="alert alert-danger" style={{ fontSize: 10, padding: '4px 8px' }}>❌ {errors[img.id]}</div>}
                    </div>
                  )
                })}
              </div>
              <div className="divider" style={{ marginTop: 14 }} />
            </div>
          )
        })()}

        {/* ─── Part 2: System Images Manager ─── */}
        <div className="section">
          <div className="section-title"><Download size={14} style={{ marginRight: 6 }} /> Browse & Download OS Images</div>

          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12, marginBottom: 12, flexWrap: 'wrap' }}>
            {/* Tabs */}
            <div className="tabs" style={{ margin: 0 }}>
              {TABS.map(tab => (
                <button
                  key={tab.id}
                  className={`tab-btn ${activeTab === tab.id ? 'active' : ''}`}
                  onClick={() => setActiveTab(tab.id)}
                  style={{ padding: '6px 10px', fontSize: 11 }}
                >
                  <span style={{ marginRight: 4 }}>{tab.icon}</span>
                  {tab.label}
                </button>
              ))}
            </div>

            <div className="flex gap-2">
              {!status?.licenses_accepted && (
                <button className="btn btn-ghost btn-sm" style={{ fontSize: 10, padding: '5px 10px' }} onClick={acceptLicenses} disabled={!!installing._licenses}>
                  {installing._licenses ? <Spinner size={10} /> : <><ShieldCheck size={10} style={{ marginRight: 4 }} /> Accept SDK Licenses</>}
                </button>
              )}
              <button className="btn btn-ghost btn-sm" style={{ fontSize: 10, padding: '5px 10px' }} onClick={loadSdkImages} disabled={loadingImages}>
                {loadingImages ? <Spinner size={10} /> : <><RefreshCw size={10} style={{ marginRight: 4 }} /> Sync Repo</>}
              </button>
            </div>
          </div>

          {/* Search */}
          <div className="search-input-wrapper" style={{ marginBottom: 12, display: 'flex', alignItems: 'center', gap: 8 }}>
            <Search size={12} style={{ color: 'var(--text-muted)' }} />
            <input
              className="form-input search-input"
              placeholder="Filter system images (e.g. '35', 'google_apis', 'playstore')..."
              value={search}
              onChange={e => setSearch(e.target.value)}
              style={{ padding: '6px 10px', fontSize: 12, width: '100%' }}
            />
          </div>

          {/* Dynamic Package List */}
          {loadingImages ? (
            <div className="flex items-center justify-center" style={{ padding: 40, gap: 12 }}>
              <Spinner size={16} />
              <span className="text-muted" style={{ fontSize: 12 }}>Fetching latest package manifests from Google APIs...</span>
            </div>
          ) : filteredImages.length === 0 ? (
            <div className="empty-state" style={{ padding: '24px 10px' }}>
              <div className="empty-icon" style={{ fontSize: 24, display: 'flex', alignItems: 'center', justifyContent: 'center' }}><PackageOpen size={24} /></div>
              <div className="empty-title" style={{ fontSize: 12 }}>No images found matching criteria</div>
              <div className="empty-desc" style={{ fontSize: 11 }}>Try clicking "Sync Repo" or clearing your search filter.</div>
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10, maxHeight: '55vh', overflowY: 'auto', paddingRight: 4 }}>
              {sortedGroupKeys.map(groupKey => {
                const groupImages = groups[groupKey];
                const isExpanded = !!expandedGroups[groupKey];
                const installedCount = groupImages.filter(img => installedList.includes(img.id) || img.installed).length;

                // Format Title Dynamically
                const firstDetails = parseImageDetails(groupImages[0]);
                const versionTitle = `Android ${firstDetails.androidVer} (API ${groupKey})`;

                return (
                  <div key={groupKey} style={{
                    background: 'rgba(255,255,255,0.01)',
                    borderRadius: 6,
                    border: '1px solid var(--border)',
                    overflow: 'hidden'
                  }}>
                    {/* Accordion Header */}
                    <div
                      onClick={() => setExpandedGroups(s => ({ ...s, [groupKey]: !s[groupKey] }))}
                      style={{
                        padding: '8px 12px',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        cursor: 'pointer',
                        background: isExpanded ? 'rgba(255,255,255,0.02)' : 'transparent',
                        transition: 'background 0.15s ease',
                        userSelect: 'none'
                      }}
                    >
                      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                        <HardDrive size={12} />
                        <span style={{ fontWeight: 600, fontSize: 12, color: isExpanded ? 'var(--text-accent)' : 'var(--text-primary)' }}>
                          {versionTitle}
                        </span>
                        <span className="badge" style={{ fontSize: 8, padding: '1px 4px', background: 'rgba(255,255,255,0.03)', color: 'var(--text-secondary)' }}>
                          {groupImages.length} {groupImages.length === 1 ? 'image' : 'images'}
                        </span>
                        {installedCount > 0 && (
                          <span className="badge badge-ok" style={{ fontSize: 8, padding: '1px 4px' }}>
                            {installedCount} Downloaded
                          </span>
                        )}
                      </div>
                      <span style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        color: 'var(--text-muted)'
                      }}>
                        {isExpanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
                      </span>
                    </div>

                    {/* Accordion Body */}
                    {isExpanded && (
                      <div style={{
                        padding: '8px 10px',
                        display: 'flex',
                        flexDirection: 'column',
                        gap: 6,
                        background: 'rgba(0,0,0,0.1)',
                        borderTop: '1px solid var(--border)'
                      }}>
                        {groupImages.map(img => {
                          const isInstalled = img.installed || installedList.includes(img.id);
                          const isInstalling = installing[img.id];
                          const pct = progress[img.id] || 0;
                          const details = parseImageDetails(img);

                          return (
                            <div key={img.id} style={{ display: 'flex', flexDirection: 'column', gap: 3 }}>
                              <div className="pkg-row" style={{ padding: '6px 8px', background: 'transparent' }}>
                                <div style={{ flex: 1 }}>
                                  <div className="pkg-name" style={{ fontSize: 11, fontWeight: 600 }}>
                                    {details.services}
                                    {isInstalled && <span className="badge badge-ok" style={{ marginLeft: 8, padding: '1px 4px', fontSize: 8 }}>Ready</span>}
                                    {details.is16k && <span className="badge badge-error" style={{ marginLeft: 4, padding: '1px 4px', fontSize: 8 }}>16 KB</span>}
                                  </div>
                                  <div className="font-mono text-muted" style={{ fontSize: 8, marginTop: 1 }}>{img.id}</div>
                                </div>
                                {isInstalled ? (
                                  <button className="btn btn-sm btn-ghost" style={{ padding: '2px 6px', fontSize: 9, color: 'var(--text-green)', borderColor: 'transparent', opacity: 0.8 }} disabled>
                                    <CheckCircle2 size={10} style={{ marginRight: 4 }} /> Ready
                                  </button>
                                ) : (
                                  <button className={`btn btn-sm ${isInstalling ? 'btn-ghost' : 'btn-primary'}`}
                                    onClick={() => installPkg(img.id)} disabled={!!isInstalling}
                                    style={{ padding: '4px 8px', fontSize: 10 }}>
                                    {isInstalling ? <Spinner size={8} /> : <><Download size={10} style={{ marginRight: 4 }} /> Download</>}
                                  </button>
                                )}
                              </div>
                              {isInstalling && (
                                <div style={{ display: 'flex', flexDirection: 'column', gap: 2, width: '100%', padding: '0 6px 4px 6px' }}>
                                  <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 9, color: 'var(--text-accent)' }}>
                                    <span>Downloading system image...</span>
                                    <span className="font-mono">{pct}%</span>
                                  </div>
                                  <div className="progress-wrap" style={{ height: 3 }}>
                                    <div className="progress-bar" style={{ width: `${pct}%`, background: 'var(--text-accent)' }} />
                                  </div>
                                </div>
                              )}
                              {errors[img.id] && <div className="alert alert-danger" style={{ fontSize: 9, padding: '4px 6px' }}>❌ {errors[img.id]}</div>}
                            </div>
                          )
                        })}
                      </div>
                    )}
                  </div>
                )
              })}
            </div>
          )}
        </div>

      </div>
      {confirmDialog}
    </>
  )
}
