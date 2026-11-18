import { Link, useParams } from 'react-router-dom'
import { CartesianGrid, Line, LineChart, ReferenceLine, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts'
import { AXIS, GRID, Legend, TooltipBox } from '../components/charts'
import { DataTable } from '../components/DataTable'
import { FilterBar } from '../components/FilterBar'
import { SplitPoCard } from '../components/SplitPo'
import { AsyncView, Card, Empty, KpiTile, PageHeader, ScoreLabel } from '../components/ui'
import { useApp } from '../context'
import { dateLabel, monthLabel, num, pct, rp, rpShort } from '../format'
import { useApi } from '../hooks'
import type { Filter, VendorScore } from '../types'

interface Po {
  po_no: string; po_date: string; item: string; category: string; qty: number; unit: string; unit_price: number; amount: number
  promised_date: string; delivered_date: string | null; late_days: number | null
}
interface Data {
  filter: Filter
  vendor: { id: number; name: string }
  score: VendorScore | null
  trend: { month: string; total: number | null; on_time_pct: number | null; quality_score: number | null; price_score: number | null }[]
  pos: Po[]
  feedback: { period: string; quality_score: number; reject_pct: number; feedback: string | null }[]
}

export default function VendorDetailPage() {
  const { id } = useParams()
  const { filterQs } = useApp()
  const state = useApi<Data>(`/vendors/${id}${filterQs}`)
  const splitQs = (() => {
    const p = new URLSearchParams(filterQs.replace(/^\?/, ''))
    p.set('vendor', String(id))
    p.delete('category')
    return `?${p.toString()}`
  })()
  return (
    <>
      <PageHeader title={state.data ? state.data.vendor.name : 'Vendor'} subtitle={<Link to="/vendor" className="hover:underline">← Kembali ke scorecard</Link>} />
      <FilterBar showVendor={false} showCategory={false} />
      <AsyncView state={state} height="h-96">
        {(d) => {
          const s = d.score
          const late = d.pos.filter((p) => p.late_days)
          return (
            <div className="flex flex-col gap-4">
              {!s ? (
                <Empty title="Tidak ada PO vendor ini pada periode terpilih" />
              ) : (
                <div className="grid grid-cols-2 gap-3 xl:grid-cols-5">
                  <KpiTile label="Skor total" value={num(s.total, 1)} delta={<ScoreLabel label={s.label} />} />
                  <KpiTile label="Ketepatan kirim" value={pct(s.on_time_pct)} hint={`${num(s.late)} dari ${num(s.delivered)} PO telat`} />
                  <KpiTile label="Skor kualitas" value={num(s.quality_score, 1)} hint={s.avg_quality ? `Rata-rata ${num(s.avg_quality, 2)} / 5 · reject ${pct(s.reject_pct)}` : 'Belum ada penilaian'} />
                  <KpiTile label="Skor harga" value={num(s.price_score, 1)} hint={`${pct(s.price_diff_pct, 1, true)} vs rata-rata pasar internal`} />
                  <KpiTile label="Spend" value={rpShort(s.spend)} hint={`${num(s.po_count)} PO`} />
                </div>
              )}
              <Card
                title="Tren skor bulanan"
                subtitle="12 bulan · garis putus = batas label Baik (80) dan Bermasalah (60)"
                action={<Legend items={[{ label: 'Skor total', color: 'var(--series-1)' }, { label: 'Ketepatan kirim', color: 'var(--series-2)' }, { label: 'Kualitas', color: 'var(--series-3)' }]} />}
              >
                <div className="h-64">
                  <ResponsiveContainer>
                    <LineChart data={d.trend} margin={{ top: 8, right: 16, left: 0, bottom: 0 }}>
                      <CartesianGrid {...GRID} />
                      <XAxis dataKey="month" tickFormatter={monthLabel} {...AXIS} />
                      <YAxis domain={[0, 100]} width={36} {...AXIS} />
                      <ReferenceLine y={80} stroke="var(--muted)" strokeDasharray="3 3" />
                      <ReferenceLine y={60} stroke="var(--muted)" strokeDasharray="3 3" />
                      <Tooltip
                        content={({ active, payload, label }) => {
                          if (!active || !payload?.length) return null
                          const p = payload[0].payload as Data['trend'][number]
                          return (
                            <TooltipBox
                              title={monthLabel(String(label))}
                              rows={[
                                { label: 'Skor total', value: num(p.total, 1), color: 'var(--series-1)' },
                                { label: 'Ketepatan kirim', value: pct(p.on_time_pct), color: 'var(--series-2)' },
                                { label: 'Kualitas', value: num(p.quality_score, 1), color: 'var(--series-3)' },
                                { label: 'Harga', value: num(p.price_score, 1) },
                              ]}
                            />
                          )
                        }}
                      />
                      <Line dataKey="total" stroke="var(--series-1)" strokeWidth={2.5} dot={{ r: 3 }} connectNulls isAnimationActive={false} />
                      <Line dataKey="on_time_pct" stroke="var(--series-2)" strokeWidth={1.5} dot={false} connectNulls isAnimationActive={false} />
                      <Line dataKey="quality_score" stroke="var(--series-3)" strokeWidth={1.5} dot={false} connectNulls isAnimationActive={false} />
                    </LineChart>
                  </ResponsiveContainer>
                </div>
              </Card>
              <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
                <Card title={`Daftar PO (${d.pos.length}${d.pos.length === 200 ? '+' : ''})`} subtitle={`${late.length} baris terlambat ditandai`} className="xl:col-span-2">
                  <DataTable<Po>
                    rows={d.pos}
                    rowKey={(r, i) => `${r.po_no}-${i}`}
                    maxHeight="420px"
                    rowClass={(r) => (r.late_days ? 'bg-surface-2' : '')}
                    columns={[
                      { key: 'n', header: 'No. PO', render: (r) => r.po_no, sort: (r) => r.po_no },
                      { key: 'd', header: 'Tanggal', render: (r) => dateLabel(r.po_date), sort: (r) => r.po_date },
                      { key: 'i', header: 'Item', render: (r) => r.item, sort: (r) => r.item },
                      { key: 'q', header: 'Qty', align: 'right', render: (r) => `${num(r.qty)} ${r.unit}` },
                      { key: 'a', header: 'Nilai', align: 'right', render: (r) => rp(r.amount), sort: (r) => r.amount },
                      { key: 'p', header: 'Janji kirim', render: (r) => dateLabel(r.promised_date), sort: (r) => r.promised_date },
                      {
                        key: 'k',
                        header: 'Terkirim',
                        render: (r) =>
                          r.delivered_date ? (
                            <span className={r.late_days ? 'font-medium text-crit' : ''}>
                              {dateLabel(r.delivered_date)}
                              {r.late_days ? ` (telat ${r.late_days} hr)` : ''}
                            </span>
                          ) : (
                            <span className="text-muted">belum</span>
                          ),
                        sort: (r) => r.late_days ?? 0,
                      },
                    ]}
                  />
                </Card>
                <Card title="Feedback terbaru" subtitle="Penilaian vendor bulanan">
                  {d.feedback.length === 0 ? (
                    <Empty title="Belum ada penilaian" />
                  ) : (
                    <ul className="divide-y divide-line">
                      {d.feedback.map((f) => (
                        <li key={f.period} className="py-2 text-sm">
                          <div className="flex justify-between text-xs text-ink-2">
                            <span>{monthLabel(f.period)}</span>
                            <span>
                              skor {num(f.quality_score, 1)} · reject {pct(f.reject_pct)}
                            </span>
                          </div>
                          <div className="text-ink">{f.feedback || '–'}</div>
                        </li>
                      ))}
                    </ul>
                  )}
                </Card>
              </div>
              <SplitPoCard qs={splitQs} showVendor={false} />
            </div>
          )
        }}
      </AsyncView>
    </>
  )
}
