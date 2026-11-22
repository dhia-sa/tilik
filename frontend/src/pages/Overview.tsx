import type { ReactNode } from 'react'
import { useNavigate, Link } from 'react-router-dom'
import { ArrowRight, Bot, CalendarRange, Database, FileText, Scissors, Star, Store, TrendingUp, Truck, Wallet } from 'lucide-react'
import { FilterBar } from '../components/FilterBar'
import { AlertKind, AsyncView, Badge, Card, DeltaBadge, Empty, IconSquare, KpiTile, PageHeader } from '../components/ui'
import { useApp } from '../context'
import { change, dateLabel, monthLabel, num, pct, rp, rpShort } from '../format'
import { useApi } from '../hooks'
import type { AlertRow, CategorySpend, Filter, Kpi, MonthPoint, SplitGroup, VendorScore } from '../types'

interface OverviewData {
  filter: Filter
  kpi: Kpi
  prev_kpi: Kpi
  trend: MonthPoint[]
  alerts: AlertRow[]
  by_category: CategorySpend[]
}

/** Daftar alert: badge minggu, deskripsi, tombol Tinjau. Dipakai juga di halaman lain. */
export function AlertList({ alerts }: { alerts: AlertRow[] }) {
  const nav = useNavigate()
  if (alerts.length === 0) return <Empty title="Tidak ada alert" hint="Harga dan vendor dalam batas normal" />
  return (
    <ul>
      {alerts.map((a) => {
        const go = () => nav(a.kind === 'price' ? `/komoditas/${a.ref_id}` : `/vendor/${a.ref_id}`)
        return (
          <li key={a.id} className="flex items-center gap-4 border-b border-line px-6 py-5 transition-colors duration-200 last:border-0 hover:bg-bg">
            <span className="hidden w-20 shrink-0 whitespace-nowrap rounded-lg bg-surface-2 px-2 py-1 text-center text-xs font-medium tabular-nums text-ink-2 sm:inline">{a.week}</span>
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <span className="font-medium text-ink">{a.ref_name}</span>
                <AlertKind kind={a.kind} />
              </div>
              <div className="mt-0.5 text-[13px] text-ink-2">{a.message}</div>
              <div className="text-xs text-muted">Minggu mulai {dateLabel(a.period_start)}</div>
            </div>
            <button onClick={go} className="h-8 shrink-0 rounded-[10px] bg-accent px-3 text-[13px] font-medium text-white hover:bg-accent-hover">
              Tinjau
            </button>
          </li>
        )
      })}
    </ul>
  )
}

/** Grafik batang CSS: kolom membulat di atas, gradien merah, membesar sedikit saat hover. */
function SpendBars({ trend, inRange }: { trend: MonthPoint[]; inRange: (m: string) => boolean }) {
  const max = Math.max(...trend.map((t) => t.spend), 1)
  return (
    <div>
      <div className="flex h-64 items-end justify-between gap-3">
        {trend.map((t) => {
          const h = Math.max((t.spend / max) * 100, t.spend > 0 ? 2 : 0)
          const on = inRange(t.month)
          return (
            <div key={t.month} className="group relative h-full w-full rounded-t-lg bg-bg" title={`${monthLabel(t.month)}: ${rp(t.spend)} · ${num(t.po_count)} PO`}>
              <div
                className={`absolute inset-x-0 bottom-0 origin-bottom rounded-t-lg transition-all duration-300 ease-[cubic-bezier(0.4,0,0.2,1)] group-hover:scale-y-105 ${
                  on ? 'bg-gradient-to-b from-[#f87171] to-[#ef4444] group-hover:from-[#ef4444] group-hover:to-[#dc2626]' : 'bg-gradient-to-b from-[#fee2e2] to-[#fecaca] dark:from-[#450a0a] dark:to-[#7f1d1d]'
                }`}
                style={{ height: `${h}%` }}
              />
              <span
                className="pointer-events-none absolute left-1/2 z-10 -translate-x-1/2 whitespace-nowrap rounded-lg bg-strong px-2 py-1 text-[11px] font-medium text-on-strong opacity-0 transition-opacity duration-200 group-hover:opacity-100"
                style={{ bottom: `calc(${h}% + 8px)` }}
              >
                {rpShort(t.spend)}
              </span>
            </div>
          )
        })}
      </div>
      <div className="mt-2 flex justify-between gap-3">
        {trend.map((t) => (
          <span key={t.month} className={`w-full text-center text-xs ${inRange(t.month) ? 'font-medium text-ink-2' : 'text-muted'}`}>
            {monthLabel(t.month).split(' ')[0]}
          </span>
        ))}
      </div>
    </div>
  )
}

function Stars({ score }: { score: number }) {
  const n = Math.max(0, Math.min(5, Math.round(score / 20)))
  return (
    <span className="flex gap-0.5 text-[#facc15]" aria-label={`${n} dari 5 bintang`}>
      {[0, 1, 2, 3, 4].map((i) => (
        <Star key={i} size={16} fill={i < n ? 'currentColor' : 'none'} strokeWidth={1.5} aria-hidden />
      ))}
    </span>
  )
}

function BestVendor() {
  const { filterQs } = useApp()
  const state = useApi<{ vendors: VendorScore[] }>(`/vendors/scorecard${filterQs}`)
  const best = [...(state.data?.vendors ?? [])].filter((v) => v.total !== null).sort((a, b) => (b.total ?? 0) - (a.total ?? 0))[0]
  return (
    <Card title="Vendor terbaik" action={best && <Badge tone="good">Skor {num(best.total, 0)}</Badge>}>
      {!state.data ? (
        <div className="h-28 animate-pulse rounded-xl bg-surface-2" />
      ) : !best ? (
        <p className="text-sm text-ink-2">Belum ada vendor dengan skor pada periode ini.</p>
      ) : (
        <>
          <div className="flex items-center gap-3">
            <IconSquare icon={<Store size={20} strokeWidth={1.75} />} tone="red" />
            <div className="min-w-0">
              <div className="truncate font-medium text-ink">{best.vendor}</div>
              <Stars score={best.total ?? 0} />
            </div>
          </div>
          <p className="my-3 text-xs italic text-ink-2">
            "{best.on_time_pct !== null ? `${pct(best.on_time_pct, 0)} PO tepat waktu` : 'Belum ada pengiriman'}
            {best.avg_quality !== null ? `, kualitas rata-rata ${num(best.avg_quality, 1)}/5` : ''}
            {best.price_diff_pct !== null ? `, harga ${pct(Math.abs(best.price_diff_pct), 1)} ${best.price_diff_pct <= 0 ? 'di bawah' : 'di atas'} rata-rata pasar internal` : ''}
            ."
          </p>
          <Link to={`/vendor/${best.vendor_id}`} className="inline-flex items-center gap-1 text-[13px] font-medium text-accent hover:text-accent-hover">
            Lihat scorecard <ArrowRight size={16} aria-hidden />
          </Link>
        </>
      )}
    </Card>
  )
}

function SplitPoPromo() {
  const { filterQs } = useApp()
  const nav = useNavigate()
  const state = useApi<{ params: { approval_limit: number }; groups: SplitGroup[] }>(`/vendors/split-po${filterQs}`)
  const groups = state.data?.groups ?? []
  const largest = groups.reduce((m, g) => Math.max(m, g.total), 0)
  return (
    <div className="rounded-2xl bg-[#0f172a] p-6 text-white dark:border dark:border-[#334155]">
      <div className="mb-4 flex items-center justify-between">
        <span className="grid h-10 w-10 place-items-center rounded-[10px] bg-[#ef4444]/15 text-[#f87171]">
          <Scissors size={20} strokeWidth={1.75} aria-hidden />
        </span>
        {groups.length > 0 ? (
          <span className="inline-flex items-center gap-1.5 rounded-full bg-[#22c55e]/15 px-2.5 py-1 text-xs font-medium text-[#4ade80]">
            <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-[#22c55e]" aria-hidden />
            Aktif
          </span>
        ) : (
          <span className="rounded-full bg-white/10 px-2.5 py-1 text-xs font-medium text-[#cbd5e1]">Bersih</span>
        )}
      </div>
      <div className="font-display text-lg font-bold">Kemungkinan split PO</div>
      <p className="mt-1 text-[13px] text-[#94a3b8]">
        Banyak PO kecil ke vendor yang sama di hari yang sama, masing-masing di bawah batas approval {state.data ? rpShort(state.data.params.approval_limit) : '…'} tetapi totalnya di atas batas.
      </p>
      <div className="mt-4 flex gap-6">
        <div>
          <div className="font-display text-[22px] font-bold">{state.data ? num(groups.length) : '…'}</div>
          <div className="text-[13px] text-[#94a3b8]">grup</div>
        </div>
        <div>
          <div className="font-display text-[22px] font-bold">{groups.length ? rpShort(largest) : '–'}</div>
          <div className="text-[13px] text-[#94a3b8]">grup terbesar</div>
        </div>
      </div>
      <button
        onClick={() => nav('/vendor#split-po')}
        className="mt-5 h-10 w-full rounded-xl border border-white/20 text-sm font-medium text-white hover:bg-white hover:text-[#0f172a]"
      >
        Tinjau kasus
      </button>
    </div>
  )
}

interface Batch { id: number; file_name: string; rows_ok: number; rows_error: number; created_at: string; rolled_back: boolean }

function DataStatus() {
  const { meta, user } = useApp()
  const canImport = user.role !== 'viewer'
  const batches = useApi<{ batches: Batch[] }>(canImport ? '/imports' : null)
  const last = batches.data?.batches.find((b) => !b.rolled_back)
  const items: { icon: ReactNode; tone: 'green' | 'blue' | 'amber' | 'slate'; title: string; sub: string }[] = [
    { icon: <CalendarRange size={18} />, tone: 'green', title: 'Rentang data', sub: `${dateLabel(meta.data_from)} – ${dateLabel(meta.data_to)}` },
    ...(canImport
      ? [{ icon: <Database size={18} />, tone: 'slate' as const, title: 'Import terakhir', sub: last ? `${last.file_name} · ${num(last.rows_ok)} baris · ${num(last.rows_error)} error` : batches.data ? 'Belum ada import' : 'Memuat…' }]
      : []),
    { icon: <Bot size={18} />, tone: 'blue', title: 'Asisten AI', sub: meta.ai_provider === 'mock' ? 'Mode mock (tanpa CLI)' : `${meta.ai_provider} CLI siap` },
    { icon: <TrendingUp size={18} />, tone: 'amber', title: 'Proyeksi harga', sub: '4 minggu ke depan, di halaman Komoditas' },
  ]
  return (
    <Card title="Status data">
      <div className="-mt-1">
        {items.map((it) => (
          <div key={it.title} className="flex items-center gap-3 border-b border-line py-3 last:border-0 last:pb-0">
            <IconSquare icon={it.icon} tone={it.tone} size={36} round />
            <div className="min-w-0">
              <div className="text-sm font-medium text-ink">{it.title}</div>
              <div className="truncate text-[13px] text-ink-2">{it.sub}</div>
            </div>
          </div>
        ))}
      </div>
    </Card>
  )
}

export default function OverviewPage() {
  const { filterQs } = useApp()
  const state = useApi<OverviewData>(`/overview${filterQs}`)
  const nav = useNavigate()
  const sub = state.data
    ? `${dateLabel(state.data.filter.from)} – ${dateLabel(state.data.filter.to)} · dibandingkan ${dateLabel(state.data.filter.prev_from)} – ${dateLabel(state.data.filter.prev_to)}`
    : 'Spend, ketepatan kirim vendor, dan alert terbaru'
  return (
    <>
      <PageHeader title="Ringkasan pengadaan" subtitle={sub} />
      <FilterBar />
      <AsyncView state={state} height="h-96">
        {(d) => {
          const inRange = (m: string) => m >= d.filter.from.slice(0, 7) && m <= d.filter.to.slice(0, 7)
          const maxCat = Math.max(...d.by_category.map((x) => x.spend), 1)
          return (
            <div className="flex flex-col gap-6">
              <section className="grid grid-cols-1 gap-6 sm:grid-cols-2 xl:grid-cols-4">
                <KpiTile
                  label="Total spend"
                  value={rpShort(d.kpi.total_spend)}
                  icon={<Wallet size={20} strokeWidth={1.75} />}
                  tone="red"
                  badge={<DeltaBadge value={change(d.kpi.total_spend, d.prev_kpi.total_spend)} invert />}
                  hint={rp(d.kpi.total_spend)}
                />
                <KpiTile
                  label="Jumlah PO"
                  value={num(d.kpi.po_count)}
                  icon={<FileText size={20} strokeWidth={1.75} />}
                  tone="blue"
                  badge={<DeltaBadge value={change(d.kpi.po_count, d.prev_kpi.po_count)} invert />}
                />
                <KpiTile
                  label="Vendor aktif"
                  value={num(d.kpi.active_vendors)}
                  icon={<Store size={20} strokeWidth={1.75} />}
                  tone="green"
                  badge={<DeltaBadge value={change(d.kpi.active_vendors, d.prev_kpi.active_vendors)} />}
                />
                <KpiTile
                  label="Rata-rata on-time delivery"
                  value={pct(d.kpi.on_time_pct)}
                  icon={<Truck size={20} strokeWidth={1.75} />}
                  tone="amber"
                  badge={
                    d.kpi.on_time_pct !== null && d.prev_kpi.on_time_pct !== null ? (
                      <DeltaBadge value={d.kpi.on_time_pct - d.prev_kpi.on_time_pct} points />
                    ) : undefined
                  }
                  hint={`${num(d.kpi.delivered_po)} PO sudah terkirim`}
                />
              </section>

              <section className="grid grid-cols-1 gap-6 xl:grid-cols-3">
                <div className="flex min-w-0 flex-col gap-6 xl:col-span-2">
                  <Card
                    flush
                    title="Alert terbaru"
                    action={
                      <Link to="/komoditas" className="inline-flex items-center gap-1 text-[13px] font-medium text-accent hover:text-accent-hover">
                        Lihat semua <ArrowRight size={16} aria-hidden />
                      </Link>
                    }
                  >
                    <AlertList alerts={d.alerts} />
                  </Card>

                  <Card
                    title="Spend per bulan"
                    subtitle="12 bulan terakhir · arahkan kursor ke batang untuk nilainya"
                    action={
                      <div className="flex gap-4 text-xs text-ink-2">
                        <span className="inline-flex items-center gap-1.5">
                          <span className="h-2.5 w-2.5 rounded-[3px] bg-[#ef4444]" />
                          Periode terpilih
                        </span>
                        <span className="inline-flex items-center gap-1.5">
                          <span className="h-2.5 w-2.5 rounded-[3px] bg-[#fecaca] dark:bg-[#7f1d1d]" />
                          Bulan lain
                        </span>
                      </div>
                    }
                  >
                    <SpendBars trend={d.trend} inRange={inRange} />
                  </Card>

                  <Card title="Spend per kategori" subtitle="Klik kategori untuk melihat detail">
                    {d.by_category.every((c) => c.spend === 0) ? (
                      <Empty hint="Tidak ada PO pada periode dan filter ini" />
                    ) : (
                      <div className="-mx-2 flex flex-col">
                        {d.by_category.map((c) => (
                          <button
                            key={c.id}
                            onClick={() => nav(`/spend/${c.id}`)}
                            className="grid grid-cols-[minmax(0,140px)_1fr_auto_auto] items-center gap-4 rounded-xl px-2 py-2.5 text-left text-sm hover:bg-bg"
                          >
                            <span className="truncate font-medium text-ink">{c.name}</span>
                            <span className="h-2 rounded-full bg-surface-2">
                              <span className="block h-2 rounded-full bg-accent" style={{ width: `${(c.spend / maxCat) * 100}%` }} />
                            </span>
                            <span className="w-24 text-right tabular-nums text-ink">{rpShort(c.spend)}</span>
                            <span className="w-20 text-right">
                              <DeltaBadge value={c.change_pct} invert />
                            </span>
                          </button>
                        ))}
                      </div>
                    )}
                  </Card>
                </div>

                <div className="flex min-w-0 flex-col gap-6">
                  <BestVendor />
                  <SplitPoPromo />
                  <DataStatus />
                </div>
              </section>
            </div>
          )
        }}
      </AsyncView>
    </>
  )
}
