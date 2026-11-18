import { useMemo } from 'react'
import { Link, useParams } from 'react-router-dom'
import { Area, CartesianGrid, ComposedChart, Line, LineChart, ReferenceDot, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts'
import { AXIS, GRID, Legend, TooltipBox } from '../components/charts'
import { DataTable } from '../components/DataTable'
import { AsyncView, Card, PageHeader } from '../components/ui'
import { num, pct, rp } from '../format'
import { useApi } from '../hooks'
import type { AlertRow, Forecast, ForecastPoint, VendorPrice, WeekDetail } from '../types'

interface Data {
  detail: { id: number; name: string; base_unit: string; weeks: WeekDetail[]; vendors: VendorPrice[]; vendor_weekly: Record<string, string | number>[]; forecast: Forecast | null }
  alerts: AlertRow[]
}

/** Baris grafik: minggu aktual + 4 minggu proyeksi. `fc`/`band` juga diisi di minggu aktual terakhir supaya garis tersambung. */
interface ChartRow {
  week: string
  week_start: string
  price: number | null
  upper_band: number | null
  change_pct: number | null
  qty: number | null
  alert: boolean
  fc: number | null
  band: [number, number] | null
  forecast: ForecastPoint | null
}

const SERIES = ['var(--series-1)', 'var(--series-2)', 'var(--series-3)', 'var(--series-4)', 'var(--series-5)']

function Detail({ d }: { d: Data }) {
  const { detail } = d
  const unit = detail.base_unit
  const vendors = detail.vendors.slice(0, 5).map((v) => v.vendor)
  const priced = detail.weeks.filter((w) => w.price !== null)
  const fc = detail.forecast
  const rows = useMemo<ChartRow[]>(() => {
    const lastPriced = [...detail.weeks].reverse().find((w) => w.price !== null)
    const actual: ChartRow[] = detail.weeks.map((w) => {
      const join = !!fc && !!lastPriced && w.week === lastPriced.week && w.price !== null
      return { ...w, fc: join ? w.price : null, band: join ? [w.price as number, w.price as number] : null, forecast: null }
    })
    const future: ChartRow[] = (fc?.points ?? []).map((p) => ({
      week: p.week,
      week_start: p.week_start,
      price: null,
      upper_band: null,
      change_pct: null,
      qty: null,
      alert: false,
      fc: p.price,
      band: [p.lower, p.upper],
      forecast: p,
    }))
    return [...actual, ...future]
  }, [detail.weeks, fc])
  const domain = useMemo(() => {
    const vals = rows.flatMap((w) => [w.price, w.upper_band, ...(w.band ?? [])]).filter((x): x is number => x !== null)
    const lo = Math.min(...vals)
    const hi = Math.max(...vals)
    const pad = (hi - lo) * 0.1 || hi * 0.05
    return [Math.floor(lo - pad), Math.ceil(hi + pad)]
  }, [rows])
  const first = priced[0]?.price ?? null
  const last = priced[priced.length - 1]?.price ?? null
  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-2 gap-3 xl:grid-cols-4">
        <div className="rounded-xl border border-line bg-surface p-4">
          <div className="text-xs text-ink-2">Harga minggu terakhir</div>
          <div className="mt-1 text-2xl font-semibold text-ink">{rp(last)}<span className="text-sm font-normal text-muted">/{unit}</span></div>
        </div>
        <div className="rounded-xl border border-line bg-surface p-4">
          <div className="text-xs text-ink-2">Perubahan 52 minggu</div>
          <div className="mt-1 text-2xl font-semibold text-ink">{first && last ? pct((last / first - 1) * 100, 1, true) : '–'}</div>
        </div>
        <div className="rounded-xl border border-line bg-surface p-4">
          <div className="text-xs text-ink-2">Minggu dengan alert</div>
          <div className="mt-1 text-2xl font-semibold text-ink">{num(detail.weeks.filter((w) => w.alert).length)}</div>
        </div>
        <div className="rounded-xl border border-line bg-surface p-4">
          <div className="text-xs text-ink-2">Vendor pemasok</div>
          <div className="mt-1 text-2xl font-semibold text-ink">{num(detail.vendors.length)}</div>
        </div>
      </div>
      <Card
        title={`Harga mingguan 52 minggu${fc ? ' + proyeksi 4 minggu' : ''}`}
        subtitle={`Rata-rata tertimbang qty (Rp/${unit}). Titik merah = minggu yang memicu alert.${fc ? ' Area arsir = rentang proyeksi 95%.' : ''}`}
        action={
          <Legend
            items={[
              { label: 'Harga rata-rata', color: 'var(--series-1)' },
              { label: 'Batas alert (rata-rata 12 mgg + kσ)', color: 'var(--muted)', dashed: true },
              ...(fc ? [{ label: 'Proyeksi', color: 'var(--series-2)', dashed: true }] : []),
            ]}
          />
        }
      >
        <div className="h-80">
          <ResponsiveContainer>
            <ComposedChart data={rows} margin={{ top: 8, right: 16, left: 8, bottom: 0 }}>
              <CartesianGrid {...GRID} />
              <XAxis dataKey="week" tickFormatter={(w: string) => w.slice(5)} interval={3} {...AXIS} />
              <YAxis domain={domain} tickFormatter={(v: number) => num(v)} width={64} {...AXIS} />
              <Tooltip
                content={({ active, payload }) => {
                  const p = payload?.[0]?.payload as ChartRow | undefined
                  if (!active || !p) return null
                  if (p.forecast) {
                    return (
                      <TooltipBox
                        title={`${p.week} (mulai ${p.week_start}) · proyeksi`}
                        rows={[
                          { label: 'Proyeksi', value: `${rp(p.forecast.price)}/${unit}`, color: 'var(--series-2)' },
                          { label: 'Rentang 95%', value: `${rp(p.forecast.lower)} – ${rp(p.forecast.upper)}` },
                        ]}
                      />
                    )
                  }
                  return (
                    <TooltipBox
                      title={`${p.week} (mulai ${p.week_start})`}
                      rows={[
                        { label: 'Harga', value: p.price ? `${rp(p.price)}/${unit}` : '–', color: 'var(--series-1)' },
                        { label: 'Δ vs minggu lalu', value: pct(p.change_pct, 1, true) },
                        { label: 'Batas alert', value: p.upper_band ? rp(p.upper_band) : '–' },
                        { label: 'Qty', value: `${num(p.qty ?? 0)} ${unit}` },
                        ...(p.alert ? [{ label: 'Status', value: '▲ Alert' }] : []),
                      ]}
                    />
                  )
                }}
              />
              {fc && <Area dataKey="band" stroke="none" fill="var(--series-2)" fillOpacity={0.18} connectNulls isAnimationActive={false} activeDot={false} />}
              {fc && <Line dataKey="fc" stroke="var(--series-2)" strokeDasharray="6 4" strokeWidth={2} dot={{ r: 3 }} connectNulls isAnimationActive={false} />}
              <Line dataKey="upper_band" stroke="var(--muted)" strokeDasharray="4 4" strokeWidth={1.5} dot={false} connectNulls isAnimationActive={false} />
              <Line dataKey="price" stroke="var(--series-1)" strokeWidth={2} dot={false} activeDot={{ r: 5 }} connectNulls isAnimationActive={false} />
              {detail.weeks
                .filter((w) => w.alert && w.price !== null)
                .map((w) => (
                  <ReferenceDot key={w.week} x={w.week} y={w.price as number} r={5} fill="var(--critical)" stroke="var(--surface)" strokeWidth={2} />
                ))}
            </ComposedChart>
          </ResponsiveContainer>
        </div>
      </Card>
      {fc ? (
        <Card
          title="Proyeksi harga 4 minggu ke depan"
          subtitle={`Regresi linear sederhana atas ${fc.points_used} minggu terakhir (s/d ${fc.last_week}). Rentang = interval prediksi 95%: makin jauh minggunya, makin lebar. Ini perkiraan statistik dari tren, bukan kepastian harga.`}
        >
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
            <div className="flex flex-col gap-1 text-sm">
              <div className="text-xs text-ink-2">Tren per minggu</div>
              <div className="text-2xl font-semibold text-ink">
                {fc.slope_per_week >= 0 ? '+' : '−'}
                {rp(Math.abs(fc.slope_per_week))}
                <span className="text-sm font-normal text-muted">/{unit}</span>
              </div>
              <div className="text-xs text-muted">
                {pct(fc.slope_pct_per_week, 2, true)} per minggu · simpangan residu {rp(fc.resid_std)}
              </div>
            </div>
            <div className="lg:col-span-2">
              <DataTable<ForecastPoint>
                rows={fc.points}
                rowKey={(r) => r.week}
                columns={[
                  { key: 'w', header: 'Minggu', render: (r) => `${r.week} (mulai ${r.week_start})` },
                  { key: 'p', header: 'Proyeksi', align: 'right', render: (r) => <span className="font-medium">{rp(r.price)}</span> },
                  { key: 'r', header: 'Rentang 95%', align: 'right', render: (r) => `${rp(r.lower)} – ${rp(r.upper)}` },
                  { key: 'c', header: `vs ${fc.last_week}`, align: 'right', render: (r) => pct((r.price / fc.last_price - 1) * 100, 1, true) },
                ]}
              />
            </div>
          </div>
        </Card>
      ) : (
        <Card title="Proyeksi harga 4 minggu ke depan">
          <p className="text-sm text-muted">Belum bisa diproyeksikan: butuh minimal 4 minggu data harga dalam 12 minggu terakhir.</p>
        </Card>
      )}
      <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
        <Card title="Harga per vendor" subtitle="Selisih dibanding rata-rata semua vendor (52 minggu)">
          <DataTable<VendorPrice>
            rows={detail.vendors}
            rowKey={(r) => r.vendor_id}
            columns={[
              { key: 'v', header: 'Vendor', render: (r) => <Link className="text-ink hover:underline" to={`/vendor/${r.vendor_id}`}>{r.vendor}</Link> },
              { key: '4', header: '4 minggu', align: 'right', render: (r) => rp(r.avg_price_4w), sort: (r) => r.avg_price_4w },
              { key: '52', header: '52 minggu', align: 'right', render: (r) => rp(r.avg_price_52w), sort: (r) => r.avg_price_52w },
              { key: 'd', header: 'vs rata-rata', align: 'right', render: (r) => pct(r.diff_vs_market_pct, 1, true), sort: (r) => r.diff_vs_market_pct },
              { key: 'q', header: 'Qty', align: 'right', render: (r) => `${num(r.qty_52w)} ${unit}`, sort: (r) => r.qty_52w },
            ]}
          />
        </Card>
        <Card title="Tren harga per vendor" action={<Legend items={vendors.map((v, i) => ({ label: v, color: SERIES[i] }))} />}>
          <div className="h-64">
            <ResponsiveContainer>
              <LineChart data={detail.vendor_weekly} margin={{ top: 8, right: 16, left: 8, bottom: 0 }}>
                <CartesianGrid {...GRID} />
                <XAxis dataKey="week" tickFormatter={(w: string) => w.slice(5)} interval={7} {...AXIS} />
                <YAxis domain={['auto', 'auto']} tickFormatter={(v: number) => num(v)} width={64} {...AXIS} />
                <Tooltip
                  content={({ active, payload, label }) => {
                    if (!active || !payload?.length) return null
                    return <TooltipBox title={String(label)} rows={payload.map((p) => ({ label: String(p.dataKey), value: rp(Number(p.value)), color: String(p.color) }))} />
                  }}
                />
                {vendors.map((v, i) => (
                  <Line key={v} dataKey={v} stroke={SERIES[i]} strokeWidth={2} dot={false} connectNulls isAnimationActive={false} />
                ))}
              </LineChart>
            </ResponsiveContainer>
          </div>
        </Card>
      </div>
    </div>
  )
}

export default function CommodityDetailPage() {
  const { id } = useParams()
  const state = useApi<Data>(`/commodities/${id}`)
  return (
    <>
      <PageHeader
        title={state.data ? `Komoditas · ${state.data.detail.name}` : 'Komoditas'}
        subtitle={<Link to="/komoditas" className="hover:underline">← Kembali ke daftar komoditas</Link>}
      />
      <AsyncView state={state} height="h-96">
        {(d) => <Detail d={d} />}
      </AsyncView>
    </>
  )
}
