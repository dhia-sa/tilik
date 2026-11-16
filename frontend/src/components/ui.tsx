import type { ReactNode } from 'react'
import { pct } from '../format'

export function Card(props: { title?: ReactNode; subtitle?: ReactNode; action?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <section className={`rounded-xl border border-line bg-surface p-4 shadow-[0_1px_2px_rgba(0,0,0,0.04)] ${props.className ?? ''}`}>
      {(props.title || props.action) && (
        <div className="mb-3 flex items-start justify-between gap-3">
          <div>
            {props.title && <h2 className="text-sm font-semibold text-ink">{props.title}</h2>}
            {props.subtitle && <p className="mt-0.5 text-xs text-muted">{props.subtitle}</p>}
          </div>
          {props.action}
        </div>
      )}
      {props.children}
    </section>
  )
}

export function PageHeader(props: { title: string; subtitle?: ReactNode; action?: ReactNode }) {
  return (
    <div className="mb-4 flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 className="text-xl font-semibold text-ink">{props.title}</h1>
        {props.subtitle && <p className="mt-1 text-sm text-ink-2">{props.subtitle}</p>}
      </div>
      {props.action}
    </div>
  )
}

type BtnVariant = 'primary' | 'secondary' | 'ghost' | 'danger'
export function Button(props: {
  children: ReactNode
  onClick?: () => void
  variant?: BtnVariant
  disabled?: boolean
  type?: 'button' | 'submit'
  className?: string
  title?: string
}) {
  const v = props.variant ?? 'secondary'
  const cls: Record<BtnVariant, string> = {
    primary: 'bg-accent text-white hover:opacity-90',
    secondary: 'border border-line bg-surface text-ink hover:bg-surface-2',
    ghost: 'text-ink-2 hover:bg-surface-2',
    danger: 'border border-line bg-surface text-crit hover:bg-surface-2',
  }
  return (
    <button
      type={props.type ?? 'button'}
      title={props.title}
      onClick={props.onClick}
      disabled={props.disabled}
      className={`inline-flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-sm font-medium transition disabled:cursor-not-allowed disabled:opacity-50 ${cls[v]} ${props.className ?? ''}`}
    >
      {props.children}
    </button>
  )
}

export function Spinner({ className = '' }: { className?: string }) {
  return <span className={`inline-block h-4 w-4 animate-spin rounded-full border-2 border-line border-t-accent ${className}`} aria-hidden />
}

export function Loading({ label = 'Memuat data…', height = 'h-40' }: { label?: string; height?: string }) {
  return (
    <div className={`flex ${height} items-center justify-center gap-2 text-sm text-muted`} role="status">
      <Spinner /> {label}
    </div>
  )
}

export function Empty({ title = 'Belum ada data', hint }: { title?: string; hint?: ReactNode }) {
  return (
    <div className="flex h-40 flex-col items-center justify-center gap-1 text-center text-sm text-muted">
      <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden>
        <path d="M3 7h18M3 12h18M3 17h10" strokeLinecap="round" />
      </svg>
      <div className="font-medium text-ink-2">{title}</div>
      {hint && <div>{hint}</div>}
    </div>
  )
}

export function ErrorBox({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-line bg-surface-2 px-4 py-3 text-sm" role="alert">
      <span className="flex items-center gap-2 text-crit">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden>
          <circle cx="12" cy="12" r="9" />
          <path d="M12 8v5M12 16h.01" strokeLinecap="round" />
        </svg>
        {message}
      </span>
      {onRetry && (
        <Button onClick={onRetry} variant="secondary">
          Coba lagi
        </Button>
      )}
    </div>
  )
}

/** Tampilkan loading / error / konten. */
export function AsyncView<T>(props: { state: { data: T | null; error: string | null; loading: boolean; reload: () => void }; children: (d: T) => ReactNode; height?: string }) {
  const { data, error, loading, reload } = props.state
  if (error) return <ErrorBox message={error} onRetry={reload} />
  if (!data) return <Loading height={props.height} />
  return (
    <div className={`relative transition-opacity ${loading ? 'opacity-60' : ''}`}>
      {loading && (
        <div className="absolute right-2 top-2 z-10">
          <Spinner />
        </div>
      )}
      {props.children(data)}
    </div>
  )
}

export function Delta({ value, invert = false, suffix = 'vs periode sebelumnya' }: { value: number | null; invert?: boolean; suffix?: string }) {
  if (value === null || Number.isNaN(value)) return <span className="text-xs text-muted">Tidak ada pembanding</span>
  const up = value > 0
  const good = invert ? !up : up
  const color = Math.abs(value) < 0.05 ? 'text-muted' : good ? 'text-good' : 'text-crit'
  return (
    <span className={`text-xs ${color}`}>
      {up ? '▲' : value < 0 ? '▼' : '■'} {pct(Math.abs(value))} <span className="text-muted">{suffix}</span>
    </span>
  )
}

export function KpiTile(props: { label: string; value: string; delta?: ReactNode; hint?: string }) {
  return (
    <div className="rounded-xl border border-line bg-surface p-4">
      <div className="text-xs font-medium text-ink-2">{props.label}</div>
      <div className="mt-1 text-2xl font-semibold tracking-tight text-ink">{props.value}</div>
      <div className="mt-1">{props.delta}</div>
      {props.hint && <div className="mt-1 text-xs text-muted">{props.hint}</div>}
    </div>
  )
}

const LABEL_STYLE: Record<string, { cls: string; icon: string }> = {
  Baik: { cls: 'text-good', icon: '●' },
  'Perlu perhatian': { cls: 'text-warn', icon: '▲' },
  Bermasalah: { cls: 'text-crit', icon: '■' },
}

export function ScoreLabel({ label }: { label: string }) {
  const s = LABEL_STYLE[label] ?? { cls: 'text-muted', icon: '○' }
  return (
    <span className={`inline-flex items-center gap-1 rounded-full border border-line bg-surface-2 px-2 py-0.5 text-xs font-medium ${s.cls}`}>
      <span aria-hidden>{s.icon}</span>
      {label}
    </span>
  )
}

export function AlertKind({ kind }: { kind: 'price' | 'vendor' }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-full border border-line bg-surface-2 px-2 py-0.5 text-[11px] font-medium text-ink-2">
      <span aria-hidden className={kind === 'price' ? 'text-crit' : 'text-warn'}>
        {kind === 'price' ? '▲' : '◆'}
      </span>
      {kind === 'price' ? 'Harga' : 'Vendor'}
    </span>
  )
}

export function Select(props: { value: string; onChange: (v: string) => void; children: ReactNode; label?: string; className?: string }) {
  return (
    <label className={`flex flex-col gap-1 text-xs text-ink-2 ${props.className ?? ''}`}>
      {props.label}
      <select
        value={props.value}
        onChange={(e) => props.onChange(e.target.value)}
        className="rounded-lg border border-line bg-surface px-2 py-1.5 text-sm text-ink outline-none focus:border-accent"
      >
        {props.children}
      </select>
    </label>
  )
}

export function Input(props: {
  value: string | number
  onChange: (v: string) => void
  label?: string
  type?: string
  placeholder?: string
  className?: string
  min?: number
  max?: number
  step?: number
}) {
  return (
    <label className={`flex flex-col gap-1 text-xs text-ink-2 ${props.className ?? ''}`}>
      {props.label}
      <input
        type={props.type ?? 'text'}
        value={props.value}
        min={props.min}
        max={props.max}
        step={props.step}
        placeholder={props.placeholder}
        onChange={(e) => props.onChange(e.target.value)}
        className="rounded-lg border border-line bg-surface px-2 py-1.5 text-sm text-ink outline-none focus:border-accent"
      />
    </label>
  )
}

export function Tabs<T extends string>(props: { value: T; onChange: (v: T) => void; options: { value: T; label: string }[] }) {
  return (
    <div className="inline-flex rounded-lg border border-line bg-surface-2 p-0.5 text-xs">
      {props.options.map((o) => (
        <button
          key={o.value}
          onClick={() => props.onChange(o.value)}
          className={`rounded-md px-2.5 py-1 font-medium ${props.value === o.value ? 'bg-surface text-ink shadow-sm' : 'text-ink-2 hover:text-ink'}`}
        >
          {o.label}
        </button>
      ))}
    </div>
  )
}
