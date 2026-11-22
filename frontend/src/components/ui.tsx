import type { ReactNode } from 'react'
import { AlertCircle, Inbox, LoaderCircle } from 'lucide-react'
import { num, pct } from '../format'
import { HeaderTools } from './HeaderTools'

/** Kartu standar: putih, border 1px, sudut 16px, tanpa bayangan. */
export function Card(props: { title?: ReactNode; subtitle?: ReactNode; action?: ReactNode; children: ReactNode; className?: string; flush?: boolean }) {
  const head = props.title || props.action
  return (
    <section className={`rounded-2xl border border-line bg-surface ${props.flush ? '' : 'p-6'} ${props.className ?? ''}`}>
      {head && (
        <div className={`flex items-start justify-between gap-3 ${props.flush ? 'border-b border-line px-6 py-5' : 'mb-4'}`}>
          <div>
            {props.title && <h2 className="text-base text-ink">{props.title}</h2>}
            {props.subtitle && <p className="mt-1 text-xs text-ink-2">{props.subtitle}</p>}
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
    <div className="flex flex-wrap items-start justify-between gap-4 pb-8">
      <div>
        <h1 className="text-2xl text-ink">{props.title}</h1>
        {props.subtitle && <p className="mt-1 text-sm text-ink-2">{props.subtitle}</p>}
      </div>
      <div className="flex flex-wrap items-center gap-3">
        {props.action}
        <HeaderTools />
      </div>
    </div>
  )
}

type BtnVariant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'dark'
export function Button(props: {
  children: ReactNode
  onClick?: () => void
  variant?: BtnVariant
  size?: 'sm' | 'md'
  disabled?: boolean
  type?: 'button' | 'submit'
  className?: string
  title?: string
}) {
  const v = props.variant ?? 'secondary'
  const cls: Record<BtnVariant, string> = {
    primary: 'bg-accent text-white hover:bg-accent-hover',
    secondary: 'border border-line bg-surface text-ink hover:border-line-strong hover:bg-bg',
    ghost: 'text-ink-2 hover:bg-surface-2 hover:text-ink',
    danger: 'border border-line bg-surface text-crit hover:bg-crit-bg',
    dark: 'bg-strong text-on-strong hover:bg-strong-hover',
  }
  const size = props.size === 'sm' ? 'h-8 rounded-[10px] px-3 text-[13px]' : 'h-10 rounded-xl px-4 text-sm'
  return (
    <button
      type={props.type ?? 'button'}
      title={props.title}
      onClick={props.onClick}
      disabled={props.disabled}
      className={`inline-flex items-center gap-2 font-medium whitespace-nowrap disabled:cursor-not-allowed disabled:opacity-50 ${size} ${cls[v]} ${props.className ?? ''}`}
    >
      {props.children}
    </button>
  )
}

export function Spinner({ className = '' }: { className?: string }) {
  return <LoaderCircle size={16} className={`animate-spin text-accent ${className}`} aria-hidden />
}

export function Loading({ label = 'Memuat data…', height = 'h-40' }: { label?: string; height?: string }) {
  return (
    <div className={`flex ${height} items-center justify-center gap-2 text-sm text-ink-2`} role="status">
      <Spinner /> {label}
    </div>
  )
}

export function Empty({ title = 'Belum ada data', hint }: { title?: string; hint?: ReactNode }) {
  return (
    <div className="flex h-40 flex-col items-center justify-center gap-1 text-center text-sm text-ink-2">
      <span className="mb-1 grid h-10 w-10 place-items-center rounded-[10px] bg-surface-2 text-muted">
        <Inbox size={20} aria-hidden />
      </span>
      <div className="font-medium text-ink">{title}</div>
      {hint && <div className="text-xs">{hint}</div>}
    </div>
  )
}

export function ErrorBox({ message, onRetry }: { message: string; onRetry?: () => void }) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-crit/20 bg-crit-bg px-4 py-3 text-sm" role="alert">
      <span className="flex items-center gap-2 text-crit">
        <AlertCircle size={16} aria-hidden />
        {message}
      </span>
      {onRetry && (
        <Button onClick={onRetry} variant="secondary" size="sm">
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
    <div className={`relative transition-opacity duration-200 ${loading ? 'opacity-60' : ''}`}>
      {loading && (
        <div className="absolute right-2 top-2 z-10">
          <Spinner />
        </div>
      )}
      {props.children(data)}
    </div>
  )
}

export type Tone = 'good' | 'warn' | 'crit' | 'info' | 'neutral'
const TONE: Record<Tone, string> = {
  good: 'bg-good-bg text-good',
  warn: 'bg-warn-bg text-warn',
  crit: 'bg-crit-bg text-crit',
  info: 'bg-info-bg text-info',
  neutral: 'bg-surface-2 text-ink-2',
}

/** Badge lembut: latar pucat + teks warna yang sama. */
export function Badge({ tone = 'neutral', children, className = '' }: { tone?: Tone; children: ReactNode; className?: string }) {
  return <span className={`inline-flex items-center gap-1 whitespace-nowrap rounded-full px-2 py-0.5 text-xs font-medium ${TONE[tone]} ${className}`}>{children}</span>
}

/** Pil status hidup dengan titik berdenyut. */
export function StatusPill({ children, tone = 'good' }: { children: ReactNode; tone?: 'good' | 'warn' }) {
  const c = tone === 'good' ? 'border-good-line bg-good-bg text-good' : 'border-warn/20 bg-warn-bg text-warn'
  const dot = tone === 'good' ? 'bg-[var(--good)]' : 'bg-[var(--warning)]'
  return (
    <span className={`inline-flex items-center gap-2 rounded-full border px-3 py-1.5 text-[13px] font-medium ${c}`}>
      <span className={`h-2 w-2 rounded-full ${dot} animate-pulse`} aria-hidden />
      {children}
    </span>
  )
}

function deltaTone(value: number, invert: boolean): Tone {
  if (Math.abs(value) < 0.05) return 'neutral'
  return (invert ? value < 0 : value > 0) ? 'good' : 'crit'
}

export function Delta({ value, invert = false, suffix = 'vs periode sebelumnya' }: { value: number | null; invert?: boolean; suffix?: string }) {
  if (value === null || Number.isNaN(value)) return <span className="text-xs text-muted">Tidak ada pembanding</span>
  const up = value > 0
  const color = { good: 'text-good', crit: 'text-crit', neutral: 'text-muted', warn: 'text-warn', info: 'text-info' }[deltaTone(value, invert)]
  return (
    <span className={`text-xs ${color}`}>
      {up ? '▲' : value < 0 ? '▼' : '■'} {pct(Math.abs(value))} {suffix && <span className="text-muted">{suffix}</span>}
    </span>
  )
}

/** Badge persentase perubahan, mis. "+12,5%". */
export function DeltaBadge({ value, invert = false, points = false }: { value: number | null; invert?: boolean; points?: boolean }) {
  if (value === null || Number.isNaN(value)) return <Badge>–</Badge>
  return (
    <Badge tone={deltaTone(value, invert)}>
      {value > 0 ? '+' : value < 0 ? '−' : ''}
      {points ? `${num(Math.abs(value), 1)} poin` : pct(Math.abs(value))}
    </Badge>
  )
}

const ICON_TONE = {
  red: 'bg-[#ef4444]/10 text-[#ef4444]',
  blue: 'bg-[#3b82f6]/10 text-[#3b82f6]',
  green: 'bg-[#22c55e]/10 text-[#22c55e]',
  amber: 'bg-[#f59e0b]/10 text-[#f59e0b]',
  violet: 'bg-[#8b5cf6]/10 text-[#8b5cf6]',
  slate: 'bg-surface-2 text-ink-2',
} as const
export type IconTone = keyof typeof ICON_TONE

export function IconSquare({ icon, tone = 'red', size = 40, round = false }: { icon: ReactNode; tone?: IconTone; size?: number; round?: boolean }) {
  return (
    <span className={`grid shrink-0 place-items-center ${round ? 'rounded-full' : 'rounded-[10px]'} ${ICON_TONE[tone]}`} style={{ width: size, height: size }}>
      {icon}
    </span>
  )
}

/** Kartu statistik: ikon kiri atas, badge kanan atas, label + nilai di bawah. */
export function KpiTile(props: { label: string; value: string; delta?: ReactNode; badge?: ReactNode; hint?: string; icon?: ReactNode; tone?: IconTone }) {
  return (
    <div className="rounded-2xl border border-line bg-surface p-6 transition-colors duration-200 hover:border-line-strong">
      {(props.icon || props.badge) && (
        <div className="mb-5 flex items-start justify-between gap-2">
          {props.icon ? <IconSquare icon={props.icon} tone={props.tone} /> : <span />}
          {props.badge}
        </div>
      )}
      <div className="text-sm text-ink-2">{props.label}</div>
      <div className="mt-0.5 font-display text-2xl font-bold tracking-tight text-ink">{props.value}</div>
      {props.delta && <div className="mt-1">{props.delta}</div>}
      {props.hint && <div className="mt-1 text-xs text-muted">{props.hint}</div>}
    </div>
  )
}

const LABEL_TONE: Record<string, Tone> = { Baik: 'good', 'Perlu perhatian': 'warn', Bermasalah: 'crit' }

export function ScoreLabel({ label }: { label: string }) {
  return <Badge tone={LABEL_TONE[label] ?? 'neutral'}>{label}</Badge>
}

export function AlertKind({ kind }: { kind: 'price' | 'vendor' }) {
  return <Badge tone={kind === 'price' ? 'crit' : 'warn'}>{kind === 'price' ? 'Harga' : 'Vendor'}</Badge>
}

const FIELD = 'h-10 rounded-xl border border-line bg-surface px-3 text-sm text-ink outline-none hover:border-line-strong focus:border-accent focus:ring-2 focus:ring-accent/15'

export function Select(props: { value: string; onChange: (v: string) => void; children: ReactNode; label?: string; className?: string }) {
  return (
    <label className={`flex flex-col gap-1.5 text-xs font-medium text-ink-2 ${props.className ?? ''}`}>
      {props.label}
      <select value={props.value} onChange={(e) => props.onChange(e.target.value)} className={FIELD}>
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
    <label className={`flex flex-col gap-1.5 text-xs font-medium text-ink-2 ${props.className ?? ''}`}>
      {props.label}
      <input
        type={props.type ?? 'text'}
        value={props.value}
        min={props.min}
        max={props.max}
        step={props.step}
        placeholder={props.placeholder}
        onChange={(e) => props.onChange(e.target.value)}
        className={FIELD}
      />
    </label>
  )
}

export function Tabs<T extends string>(props: { value: T; onChange: (v: T) => void; options: { value: T; label: string }[] }) {
  return (
    <div className="inline-flex rounded-xl border border-line bg-surface-2 p-1 text-xs">
      {props.options.map((o) => (
        <button
          key={o.value}
          onClick={() => props.onChange(o.value)}
          className={`rounded-lg px-3 py-1.5 font-medium ${props.value === o.value ? 'bg-surface text-accent ring-1 ring-line' : 'text-ink-2 hover:text-ink'}`}
        >
          {o.label}
        </button>
      ))}
    </div>
  )
}
