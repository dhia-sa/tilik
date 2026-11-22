import { useState } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { Bar, BarChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts'
import { postJson } from '../api'
import { AXIS, GRID, TooltipBox } from '../components/charts'
import { DataTable } from '../components/DataTable'
import { ExportButton } from '../components/Export'
import { FilterBar } from '../components/FilterBar'
import { AsyncView, Button, Card, Delta, Empty, ErrorBox, PageHeader, Spinner } from '../components/ui'
import { useApp } from '../context'
import { dateLabel, monthLabel, num, pct, rp, rpShort } from '../format'
import { useApi } from '../hooks'
import type { CategorySpend, Decomposition, Filter, ItemEffect, ItemSpend, MonthPoint, SpendRow } from '../types'

interface ByCat {
  filter: Filter
  categories: CategorySpend[]
  total: number
  prev_total: number
}
interface CatDetail {
  filter: Filter
  category: { id: number; name: string }
  trend: MonthPoint[]
  top_items: ItemSpend[]
  top_vendors: SpendRow[]
  decomposition: Decomposition
}
interface Explain {
  narrative: string
  fallback: boolean
  fallback_reason?: string
  provider: string
}

function Waterfall({ d }: { d: Decomposition }) {
  const steps = [
    { label: 'Spend periode sebelumnya', value: d.spend0, kind: 'total' as const },
    { label: 'Efek volume', value: d.volume_effect, kind: 'delta' as const },
    { label: 'Efek harga', value: d.price_effect, kind: 'delta' as const },
    { label: 'Item baru', value: d.new_item_effect, kind: 'delta' as const },
    { label: 'Item tidak dibeli lagi', value: d.lost_item_effect, kind: 'delta' as const },
    { label: 'Spend periode ini', value: d.spend1, kind: 'total' as const },
  ]
  let running = 0
  const bars = steps.map((s) => {
    if (s.kind === 'total') {
      running = s.value
      return { ...s, start: 0, end: s.value }
    }
    const start = running
    running += s.value
    return { ...s, start, end: running }
  })
  // Sumbu dipotong (tidak mulai dari nol) supaya efek volume/harga yang relatif kecil tetap terlihat.
  const max = Math.max(...bars.map((b) => Math.max(b.start, b.end)), 1)
  const lows = bars.flatMap((b) => (b.kind === 'total' ? [b.end] : [b.start, b.end]))
  const lowest = Math.min(...lows)
  const min = Math.max(0, lowest - (max - lowest) * 0.6)
  const span = max - min || 1
  return (
    <div className="flex flex-col gap-1.5" role="img" aria-label="Dekomposisi perubahan spend">
      {bars.map((b) => {
        const lo = b.kind === 'total' ? min : Math.min(b.start, b.end)
        const left = ((lo - min) / span) * 100
        const hi = b.kind === 'total' ? b.end : Math.max(b.start, b.end)
        const width = Math.max(((hi - lo) / span) * 100, 0.4)
        const color = b.kind === 'total' ? 'var(--series-1)' : b.value >= 0 ? 'var(--series-2)' : 'var(--series-3)'
        return (
          <div key={b.label} className="grid grid-cols-[170px_1fr_150px] items-center gap-3 text-sm">
            <span className={b.kind === 'total' ? 'font-medium text-ink' : 'text-ink-2'}>{b.label}</span>
            <span className="relative h-5 rounded bg-surface-2">
              <span className="absolute top-0 h-5 rounded" style={{ left: `${left}%`, width: `${width}%`, background: color }} />
            </span>
            <span className="text-right tabular-nums text-ink">
              {b.kind === 'delta' && b.value > 0 ? '+' : ''}
              {rpShort(b.value)}
            </span>
          </div>
        )
      })}
      <div className="mt-1 flex flex-wrap gap-4 text-xs text-ink-2">
        <span className="inline-flex items-center gap-1.5"><span className="h-2.5 w-2.5 rounded-sm" style={{ background: 'var(--series-1)' }} />Total spend</span>
        <span className="inline-flex items-center gap-1.5"><span className="h-2.5 w-2.5 rounded-sm" style={{ background: 'var(--series-2)' }} />Menambah spend</span>
        <span className="inline-flex items-center gap-1.5"><span className="h-2.5 w-2.5 rounded-sm" style={{ background: 'var(--series-3)' }} />Mengurangi spend</span>
        {min > 0 && <span className="text-muted">Sumbu dimulai dari {rpShort(min)} (bukan nol)</span>}
      </div>
    </div>
  )
}

function ExplainBox({ categoryId }: { categoryId: number }) {
  const { filterQs } = useApp()
  const [state, setState] = useState<{ loading: boolean; data: Explain | null; error: string | null }>({ loading: false, data: null, error: null })
  const run = async () => {
    setState({ loading: true, data: null, error: null })
    try {
      const d = await postJson<Explain>(`/spend/category/${categoryId}/explain${filterQs}`, {})
      setState({ loading: false, data: d, error: null })
    } catch (e) {
      setState({ loading: false, data: null, error: (e as Error).message })
    }
  }
  return (
    <div className="rounded-xl border border-line bg-surface-2 p-3">
      <div className="flex items-center justify-between gap-3">
        <div className="text-sm font-medium text-ink">Jelaskan dengan AI</div>
        <Button variant="primary" onClick={run} disabled={state.loading}>
          {state.loading ? <><Spinner /> Menganalisis…</> : state.data ? 'Ulangi' : '✦ Jelaskan'}
        </Button>
      </div>
      {state.error && <div className="mt-2"><ErrorBox message={state.error} /></div>}
      {state.data && (
        <div className="mt-2 text-sm leading-relaxed text-ink">
          {state.data.narrative}
          <div className="mt-2 text-xs text-muted">
            Sumber: dekomposisi di atas · provider {state.data.provider}
            {state.data.fallback ? ` · narasi otomatis (${state.data.fallback_reason ?? 'AI tidak tersedia'})` : ' · angka terverifikasi dengan data'}
          </div>
        </div>
      )}
      {!state.data && !state.loading && !state.error && <p className="mt-1 text-xs text-muted">Narasi singkat kenapa spend kategori ini naik/turun, berdasarkan dekomposisi harga vs volume.</p>}
    </div>
  )
}

function CategoryDetail({ id }: { id: number }) {
  const { filterQs } = useApp()
  const state = useApi<CatDetail>(`/spend/category/${id}${filterQs}`)
  return (
    <AsyncView state={state} height="h-96">
      {(d) => (
        <div className="flex flex-col gap-4">
          <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
            <Card title={`Tren bulanan · ${d.category.name}`} subtitle="12 bulan terakhir">
              <div className="h-64">
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
                    <Bar dataKey="spend" fill="var(--series-1)" radius={[4, 4, 0, 0]} maxBarSize={32} />
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </Card>
            <Card
              title="Dekomposisi perubahan: volume vs harga"
              subtitle={`${dateLabel(d.filter.from)} – ${dateLabel(d.filter.to)} vs ${dateLabel(d.filter.prev_from)} – ${dateLabel(d.filter.prev_to)}`}
            >
              <Waterfall d={d.decomposition} />
              <p className="mt-2 text-xs text-muted">
                Efek volume = (qty₁ − qty₀) × harga₀ · Efek harga = (harga₁ − harga₀) × qty₁ · dihitung per item dalam satuan dasar lalu dijumlah.
              </p>
            </Card>
          </div>
          <ExplainBox categoryId={id} />
          <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
            <Card title="Top 10 item" className="xl:col-span-2">
              <DataTable<ItemSpend>
                rows={d.top_items}
                rowKey={(r) => r.item_id}
                columns={[
                  { key: 'item', header: 'Item', render: (r) => r.item, sort: (r) => r.item },
                  { key: 'spend', header: 'Spend', align: 'right', render: (r) => rp(r.spend), sort: (r) => r.spend },
                  { key: 'qty', header: 'Qty', align: 'right', render: (r) => `${num(r.base_qty)} ${r.base_unit}`, sort: (r) => r.base_qty },
                  { key: 'avg', header: 'Harga rata-rata', align: 'right', render: (r) => `${rp(r.avg_price)}/${r.base_unit}`, sort: (r) => r.avg_price },
                  { key: 'po', header: 'PO', align: 'right', render: (r) => num(r.po_count), sort: (r) => r.po_count },
                ]}
              />
            </Card>
            <Card title="Top 5 vendor">
              <DataTable<SpendRow>
                rows={d.top_vendors}
                rowKey={(r) => r.name}
                columns={[
                  { key: 'v', header: 'Vendor', render: (r) => r.name },
                  { key: 's', header: 'Spend', align: 'right', render: (r) => rpShort(r.spend) },
                  { key: 'p', header: 'Porsi', align: 'right', render: (r) => pct(r.share_pct) },
                ]}
              />
            </Card>
          </div>
          <Card title="Kontribusi per item" subtitle="Item dengan perubahan spend terbesar (absolut)">
            <DataTable<ItemEffect>
              rows={d.decomposition.items}
              rowKey={(r) => r.item_id}
              columns={[
                { key: 'n', header: 'Item', render: (r) => r.name },
                { key: 's0', header: 'Spend sebelumnya', align: 'right', render: (r) => rpShort(r.spend0), sort: (r) => r.spend0 },
                { key: 's1', header: 'Spend sekarang', align: 'right', render: (r) => rpShort(r.spend1), sort: (r) => r.spend1 },
                { key: 'v', header: 'Efek volume', align: 'right', render: (r) => rpShort(r.volume_effect), sort: (r) => r.volume_effect },
                { key: 'h', header: 'Efek harga', align: 'right', render: (r) => rpShort(r.price_effect), sort: (r) => r.price_effect },
                {
                  key: 'p',
                  header: 'Δ harga',
                  align: 'right',
                  render: (r) => (r.price0 && r.price1 ? pct((r.price1 / r.price0 - 1) * 100, 1, true) : r.price0 ? 'tidak dibeli' : 'item baru'),
                },
                { key: 't', header: 'Total perubahan', align: 'right', render: (r) => rpShort(r.total_change), sort: (r) => r.total_change },
              ]}
            />
          </Card>
        </div>
      )}
    </AsyncView>
  )
}

export default function SpendPage() {
  const { filterQs } = useApp()
  const params = useParams()
  const nav = useNavigate()
  const selected = params.id ? Number(params.id) : null
  const state = useApi<ByCat>(`/spend/by-category${filterQs}`)
  return (
    <>
      <PageHeader title="Spend per kategori" subtitle="Kategori mana yang terbesar dan kenapa berubah." action={<ExportButton view="spend" />} />
      <FilterBar showCategory={false} />
      <AsyncView state={state}>
        {(d) => (
          <Card title="Spend per kategori" subtitle={`Total ${rp(d.total)} · klik kategori untuk drill-down`}>
            {d.total === 0 ? (
              <Empty hint="Tidak ada PO pada periode ini" />
            ) : (
              <div className="grid grid-cols-2 gap-2 lg:grid-cols-3 xl:grid-cols-6">
                {d.categories.map((c) => (
                  <button
                    key={c.id}
                    onClick={() => nav(selected === c.id ? '/spend' : `/spend/${c.id}`)}
                    className={`rounded-lg border p-3 text-left transition ${selected === c.id ? 'border-accent bg-surface-2' : 'border-line hover:bg-surface-2'}`}
                  >
                    <div className="text-xs font-medium text-ink-2">{c.name}</div>
                    <div className="mt-1 font-display text-lg font-bold tabular-nums text-ink">{rpShort(c.spend)}</div>
                    <div className="mt-1 h-1.5 rounded bg-surface-2">
                      <div className="h-1.5 rounded" style={{ width: `${c.share_pct}%`, background: 'var(--series-1)' }} />
                    </div>
                    <div className="mt-1 flex items-center justify-between text-xs">
                      <span className="text-muted">{pct(c.share_pct)} total</span>
                      <Delta value={c.change_pct} invert suffix="" />
                    </div>
                  </button>
                ))}
              </div>
            )}
          </Card>
        )}
      </AsyncView>
      <div className="mt-4">{selected ? <CategoryDetail id={selected} /> : <div className="rounded-xl border border-dashed border-line p-6 text-center text-sm text-muted">Pilih kategori di atas untuk melihat tren, top item, top vendor, dan dekomposisi harga vs volume.</div>}</div>
    </>
  )
}
