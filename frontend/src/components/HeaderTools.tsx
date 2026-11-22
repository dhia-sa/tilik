import { useEffect, useRef, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { Bell, LifeBuoy, X } from 'lucide-react'
import { useApp } from '../context'
import { dateLabel } from '../format'
import { useApi } from '../hooks'
import type { AlertRow } from '../types'

function daysBefore(d: string, n: number): string {
  const t = new Date(d + 'T00:00:00Z')
  t.setUTCDate(t.getUTCDate() - n)
  return t.toISOString().slice(0, 10)
}

/** Pojok kanan header: status data, notifikasi alert, pusat bantuan. */
export function HeaderTools() {
  const { meta } = useApp()
  return (
    <>
      <span className="inline-flex items-center gap-2 rounded-full border border-good-line bg-good-bg px-3 py-1.5 text-[13px] font-medium text-good">
        <span className="h-2 w-2 animate-pulse rounded-full bg-[var(--good)]" aria-hidden />
        Data s/d {meta.data_to ? dateLabel(meta.data_to) : '–'}
      </span>
      <AlertBell />
      <HelpButton />
    </>
  )
}

function AlertBell() {
  const { meta } = useApp()
  const nav = useNavigate()
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  const since = daysBefore(meta.reference_date, 13)
  const state = useApi<{ alerts: AlertRow[] }>(`/alerts?from=${since}&limit=8`)
  const alerts = state.data?.alerts ?? []
  useEffect(() => {
    if (!open) return
    const close = (e: MouseEvent) => ref.current && !ref.current.contains(e.target as Node) && setOpen(false)
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [open])
  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen((o) => !o)}
        aria-label={`Notifikasi, ${alerts.length} alert 2 minggu terakhir`}
        className="relative grid h-10 w-10 place-items-center rounded-xl border border-line bg-surface text-ink-2 hover:border-line-strong hover:text-ink"
      >
        <Bell size={20} strokeWidth={1.75} />
        {alerts.length > 0 && <span className="absolute right-2.5 top-2.5 h-2 w-2 rounded-full border-2 border-surface bg-accent" aria-hidden />}
      </button>
      {open && (
        <div className="absolute right-0 top-12 z-30 w-80 overflow-hidden rounded-2xl border border-line bg-surface shadow-[0_8px_24px_rgba(15,23,42,0.08)]">
          <div className="flex items-center justify-between border-b border-line px-4 py-3">
            <span className="font-display text-sm font-bold text-ink">Alert 2 minggu terakhir</span>
            <span className="text-xs text-muted">{alerts.length}</span>
          </div>
          {alerts.length === 0 ? (
            <div className="px-4 py-6 text-center text-sm text-ink-2">Harga dan vendor dalam batas normal.</div>
          ) : (
            <ul className="max-h-80 overflow-auto">
              {alerts.map((a) => (
                <li key={a.id}>
                  <button
                    onClick={() => {
                      setOpen(false)
                      nav(a.kind === 'price' ? `/komoditas/${a.ref_id}` : `/vendor/${a.ref_id}`)
                    }}
                    className="flex w-full gap-3 border-b border-line px-4 py-3 text-left last:border-0 hover:bg-bg"
                  >
                    <span className={`mt-1.5 h-2 w-2 shrink-0 rounded-full ${a.kind === 'price' ? 'bg-accent' : 'bg-[var(--warning)]'}`} aria-hidden />
                    <span className="min-w-0">
                      <span className="block text-sm text-ink">{a.message}</span>
                      <span className="text-xs text-muted">
                        {a.week} · {dateLabel(a.period_start)}
                      </span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  )
}

const HELP: { title: string; body: string }[] = [
  { title: 'Filter periode', body: 'Filter di atas tiap halaman berlaku untuk semua halaman. Angka selalu dibandingkan dengan periode sebelumnya yang panjangnya sama.' },
  { title: 'Ringkasan', body: 'KPI, tren spend bulanan, alert terbaru, vendor terbaik, dan kemungkinan split PO dalam satu layar.' },
  { title: 'Komoditas', body: 'Harga rata-rata tertimbang per minggu. Klik komoditas untuk grafik 52 minggu, harga per vendor, dan proyeksi 4 minggu.' },
  { title: 'Vendor', body: 'Scorecard ketepatan kirim, kualitas, dan harga. Bobot skor bisa diatur Data Admin di Pengaturan.' },
  { title: 'Tanya Data', body: 'Tanya dalam bahasa biasa, mis. "vendor mana yang paling sering telat bulan ini?". Setiap angka di jawaban diperiksa terhadap tabel hasil.' },
  { title: 'Import', body: 'Data Admin bisa upload CSV PO. Baris yang salah ditampilkan sebelum disimpan, dan setiap batch bisa di-rollback.' },
]

function HelpButton() {
  const [open, setOpen] = useState(false)
  useEffect(() => {
    if (!open) return
    const esc = (e: KeyboardEvent) => e.key === 'Escape' && setOpen(false)
    document.addEventListener('keydown', esc)
    return () => document.removeEventListener('keydown', esc)
  }, [open])
  return (
    <>
      <button onClick={() => setOpen(true)} className="inline-flex h-10 items-center gap-2 rounded-xl bg-strong px-4 text-sm font-medium text-on-strong hover:bg-strong-hover">
        <LifeBuoy size={18} strokeWidth={1.75} />
        Pusat bantuan
      </button>
      {open && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-[#0f172a]/40 p-4" onMouseDown={() => setOpen(false)}>
          <div role="dialog" aria-modal="true" aria-label="Pusat bantuan" className="w-full max-w-lg rounded-2xl border border-line bg-surface" onMouseDown={(e) => e.stopPropagation()}>
            <div className="flex items-center justify-between border-b border-line px-6 py-4">
              <h2 className="text-lg text-ink">Pusat bantuan</h2>
              <button onClick={() => setOpen(false)} aria-label="Tutup" className="grid h-8 w-8 place-items-center rounded-lg text-ink-2 hover:bg-surface-2 hover:text-ink">
                <X size={18} />
              </button>
            </div>
            <div className="flex max-h-[70vh] flex-col gap-4 overflow-auto px-6 py-5">
              {HELP.map((h) => (
                <div key={h.title}>
                  <div className="font-display text-sm font-bold text-ink">{h.title}</div>
                  <p className="mt-0.5 text-sm text-ink-2">{h.body}</p>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </>
  )
}
