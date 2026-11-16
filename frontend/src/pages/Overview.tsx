import { Bar, BarChart, CartesianGrid, Cell, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts'
import { useNavigate } from 'react-router-dom'
import { AXIS, GRID, TooltipBox } from '../components/charts'
import { FilterBar } from '../components/FilterBar'
import { AlertKind, AsyncView, Card, Delta, Empty, KpiTile, PageHeader } from '../components/ui'
import { useApp } from '../context'
import { change, dateLabel, monthLabel, num, pct, rp, rpShort } from '../format'
import { useApi } from '../hooks'
import type { AlertRow, CategorySpend, Filter, Kpi, MonthPoint } from '../types'

interface OverviewData {
  filter: Filter
  kpi: Kpi
  prev_kpi: Kpi
  trend: MonthPoint[]
  alerts: AlertRow[]
  by_category: CategorySpend[]
}

export function AlertList({ alerts }: { alerts: AlertRow[] }) {
  const nav = useNavigate()
  if (alerts.length === 0) return <Empty title="Tidak ada alert" hint="Harga & vendor dalam batas normal" />
  return (
    <ul className="divide-y divide-line">
      {alerts.map((a) => (
        <li
          key={a.id}
          className="flex cursor-pointer gap-3 py-2.5 hover:bg-surface-2"
          onClick={() => nav(a.kind === 'price' ? `/komoditas/${a.ref_id}` : `/vendor/${a.ref_id}`)}
        >
          <div className="w-16 shrink-0 pt-0.5">
            <AlertKind kind={a.kind} />
          </div>
          <div className="min-w-0">
            <div className="text-sm text-ink">{a.message}</div>
            <div className="text-xs text-muted">
              {a.week} · mulai {dateLabel(a.period_start)}
            </div>
          </div>
        </li>
      ))}
    </ul>
  )
}

export default function OverviewPage() {
  const { filterQs } = useApp()
  const state = useApi<OverviewData>(`/overview${filterQs}`)
  const nav = useNavigate()
  return (
    <>
      <PageHeader title="Overview" subtitle="Ringkasan spend, ketepatan kirim vendor, dan alert terbaru." />
      <FilterBar />
      <AsyncView state={state} height="h-96">
        {(d) => {
          const inRange = (m: string) => m >= d.filter.from.slice(0, 7) && m <= d.filter.to.slice(0, 7)
          return (
            <div className="flex flex-col gap-4">
              <div className="text-xs text-muted">
                Periode {dateLabel(d.filter.from)} – {dateLabel(d.filter.to)} dibandingkan {dateLabel(d.filter.prev_from)} – {dateLabel(d.filter.prev_to)}
              </div>
              <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
                <KpiTile label="Total spend" value={rpShort(d.kpi.total_spend)} delta={<Delta value={change(d.kpi.total_spend, d.prev_kpi.total_spend)} invert />} hint={rp(d.kpi.total_spend)} />
                <KpiTile label="Jumlah PO" value={num(d.kpi.po_count)} delta={<Delta value={change(d.kpi.po_count, d.prev_kpi.po_count)} invert />} />
                <KpiTile label="Vendor aktif" value={num(d.kpi.active_vendors)} delta={<Delta value={change(d.kpi.active_vendors, d.prev_kpi.active_vendors)} />} />
                <KpiTile
                  label="Rata-rata on-time delivery"
                  value={pct(d.kpi.on_time_pct)}
                  delta={
                    d.kpi.on_time_pct !== null && d.prev_kpi.on_time_pct !== null ? (
                      <Delta value={d.kpi.on_time_pct - d.prev_kpi.on_time_pct} suffix="poin vs periode sebelumnya" />
                    ) : undefined
                  }
                  hint={`${num(d.kpi.delivered_po)} PO sudah terkirim`}
                />
              </div>
              <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
                <Card title="Tren spend bulanan" subtitle="12 bulan terakhir · bulan dalam periode terpilih ditandai lebih gelap" className="xl:col-span-2">
                  <div className="h-72">
                    <ResponsiveContainer>
                      <BarChart data={d.trend} margin={{ top: 8, right: 8, left: 8, bottom: 0 }}>
                        <CartesianGrid {...GRID} />
                        <XAxis dataKey="month" tickFormatter={monthLabel} {...AXIS} />
                        <YAxis tickFormatter={(v: number) => rpShort(v).replace('Rp ', '')} width={56} {...AXIS} />
                        <Tooltip
                          cursor={{ fill: 'var(--surface-2)' }}
                          content={({ active, payload }) => {
                            const p = payload?.[0]?.payload as MonthPoint | undefined
                            if (!active || !p) return null
                            return <TooltipBox title={monthLabel(p.month)} rows={[{ label: 'Spend', value: rp(p.spend) }, { label: 'Jumlah PO', value: num(p.po_count) }]} />
                          }}
                        />
                        <Bar dataKey="spend" radius={[4, 4, 0, 0]} maxBarSize={36}>
                          {d.trend.map((t) => (
                            <Cell key={t.month} fill={inRange(t.month) ? 'var(--series-1)' : 'var(--seq-light)'} />
                          ))}
                        </Bar>
                      </BarChart>
                    </ResponsiveContainer>
                  </div>
                </Card>
                <Card title="5 alert terbaru" subtitle="Harga komoditas & performa vendor">
                  <AlertList alerts={d.alerts} />
                </Card>
              </div>
              <Card title="Spend per kategori" subtitle="Klik kategori untuk melihat detail">
                {d.by_category.every((c) => c.spend === 0) ? (
                  <Empty hint="Tidak ada PO pada periode & filter ini" />
                ) : (
                  <div className="flex flex-col gap-2">
                    {d.by_category.map((c) => {
                      const max = Math.max(...d.by_category.map((x) => x.spend), 1)
                      return (
                        <button key={c.id} onClick={() => nav(`/spend/${c.id}`)} className="grid grid-cols-[150px_1fr_130px_150px] items-center gap-3 rounded-lg px-1 py-1 text-left text-sm hover:bg-surface-2">
                          <span className="truncate text-ink">{c.name}</span>
                          <span className="h-3 rounded bg-surface-2">
                            <span className="block h-3 rounded" style={{ width: `${(c.spend / max) * 100}%`, background: 'var(--series-1)' }} />
                          </span>
                          <span className="text-right tabular-nums text-ink">{rpShort(c.spend)}</span>
                          <span className="text-right">
                            <Delta value={c.change_pct} invert suffix="" />
                          </span>
                        </button>
                      )
                    })}
                  </div>
                )}
              </Card>
            </div>
          )
        }}
      </AsyncView>
    </>
  )
}
