import { useState, useEffect, useRef, useCallback } from 'react'
import { CheckCircle2, XCircle, Info, AlertTriangle, Check, ChevronDown, AlertTriangle as WarnIcon } from 'lucide-react'

export function useToast() {
  const [toasts, setToasts] = useState([])
  const add = useCallback((msg, type = 'info') => {
    const id = Date.now()
    setToasts(t => [...t, { id, msg, type }])
    setTimeout(() => setToasts(t => t.filter(x => x.id !== id)), 3500)
  }, [])
  return { toasts, toast: add }
}

export function ToastContainer({ toasts }) {
  if (!toasts.length) return null
  const icons = {
    success: <CheckCircle2 size={16} style={{ color: 'var(--text-green)' }} />,
    error: <XCircle size={16} style={{ color: 'var(--text-pink)' }} />,
    info: <Info size={16} style={{ color: 'var(--text-cyan)' }} />,
    warn: <AlertTriangle size={16} style={{ color: '#fbbf24' }} />
  }
  return (
    <div className="toast-container">
      {toasts.map(t => (
        <div key={t.id} className={`toast ${t.type}`}>
          <span style={{ display: 'inline-flex', alignItems: 'center' }}>{icons[t.type] || <Info size={16} />}</span>
          <span>{t.msg}</span>
        </div>
      ))}
    </div>
  )
}

export function ProgressBar({ value = 0 }) {
  return (
    <div className="progress-wrap">
      <div className="progress-bar" style={{ width: `${Math.min(100, value)}%` }} />
    </div>
  )
}

export function Spinner({ size = 18 }) {
  return <div className="spinner" style={{ width: size, height: size }} />
}

export function CustomSelect({ value, onChange, options = [], placeholder = 'Select', disabled = false }) {
  const [open, setOpen] = useState(false)
  const ref = useRef(null)

  useEffect(() => {
    const handleClickOutside = (event) => {
      if (ref.current && !ref.current.contains(event.target)) {
        setOpen(false)
      }
    }

    document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [])

  const selected = options.find(opt => String(opt.value) === String(value))

  return (
    <div className="custom-select" ref={ref}>
      <button
        type="button"
        className="custom-select-trigger"
        onClick={() => !disabled && setOpen(o => !o)}
        disabled={disabled}
      >
        <span className="custom-select-label">{selected?.label || placeholder}</span>
        <ChevronDown size={14} style={{ color: 'var(--text-muted)' }} />
      </button>

      {open && (
        <div className="custom-select-menu">
          {options.map(opt => (
            <button
              key={String(opt.value)}
              type="button"
              className={`custom-select-option ${String(value) === String(opt.value) ? 'active' : ''}`}
              onClick={() => {
                onChange(opt.value)
                setOpen(false)
              }}
            >
              <span>{opt.label}</span>
              {String(value) === String(opt.value) && <Check size={12} style={{ color: 'var(--text-accent)' }} />}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

export function ConsoleLog({ lines = [] }) {
  const ref = useRef(null)
  useEffect(() => {
    if (ref.current) ref.current.scrollTop = ref.current.scrollHeight
  }, [lines])

  const classify = (line) => {
    if (/success|installed|done|completed/i.test(line)) return 'success'
    if (/error|fail|exception/i.test(line)) return 'error'
    if (/warn|warning/i.test(line)) return 'warn'
    if (/download|install|launch|starting|ready|info/i.test(line)) return 'info'
    return ''
  }

  return (
    <div className="console-wrap" ref={ref}>
      {lines.length === 0 && (
        <div className="console-line text-muted">// Console output will appear here...</div>
      )}
      {lines.map((line, i) => (
        <div key={i} className={`console-line ${classify(line)}`}>
          <span className="text-muted font-mono" style={{ userSelect: 'none', marginRight: 8 }}>
            {String(i + 1).padStart(3, '0')}
          </span>
          {line}
        </div>
      ))}
    </div>
  )
}

export function Toggle({ value, onChange, label }) {
  return (
    <label className="toggle-wrap" onClick={() => onChange(!value)} style={{ cursor: 'pointer' }}>
      <div className={`toggle ${value ? 'on' : ''}`} />
      {label && <span className="toggle-label">{label}</span>}
    </label>
  )
}

// ── ConfirmDialog & useConfirm ──────────────────────────────────────────────
// Tauri 2 with decorations:false silently blocks native window.confirm().
// Use this hook + component instead to show an in-app modal.

export function useConfirm() {
  const [state, setState] = useState(null) // { message, resolve }

  const confirm = useCallback((message) => {
    return new Promise((resolve) => {
      setState({ message, resolve })
    })
  }, [])

  const handleResponse = useCallback((result) => {
    state?.resolve(result)
    setState(null)
  }, [state])

  const dialog = state ? (
    <ConfirmDialog
      message={state.message}
      onConfirm={() => handleResponse(true)}
      onCancel={() => handleResponse(false)}
    />
  ) : null

  return { confirm, dialog }
}

export function ConfirmDialog({ message, onConfirm, onCancel }) {
  // Close on Escape key
  useEffect(() => {
    const handler = (e) => { if (e.key === 'Escape') onCancel() }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [onCancel])

  return (
    <div className="confirm-overlay" onClick={onCancel}>
      <div className="confirm-modal" onClick={e => e.stopPropagation()}>
        <div className="confirm-icon">
          <WarnIcon size={22} style={{ color: '#fbbf24' }} />
        </div>
        <p className="confirm-message">{message}</p>
        <div className="confirm-actions">
          <button className="btn btn-ghost" onClick={onCancel}>Cancel</button>
          <button className="btn btn-danger" onClick={onConfirm}>Confirm</button>
        </div>
      </div>
    </div>
  )
}
