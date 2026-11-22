import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { postJson } from '../api'
import { DataTable } from '../components/DataTable'
import { ExportButton } from '../components/Export'
import { FilterBar } from '../components/FilterBar'
import { SplitPoCard } from '../components/SplitPo'
import { AsyncView, Button, Card, ErrorBox, Input, PageHeader, ScoreLabel } from '../components/ui'
import { useApp } from '../context'
import { num, pct, rpShort } from '../format'
import { useApi } from '../hooks'
import type { Filter, VendorScore, Weights } from '../types'

interface Data {
  filter: Filter
  weights: Weights
  vendors: VendorScore[]
  problems: VendorScore[]
}

function WeightEditor({ weights, onSaved }: { weights: Weights; onSaved: () => void }) {
  const [w, setW] = useState({ on_time: String(weights.on_time), quality: String(weights.quality), price: String(weights.price) })
  const [busy, setBusy] = useState(false)
  const [err, setErr] = useState<string | null>(null)
  useEffect(() => {
    setW({ on_time: String(weights.on_time), quality: String(weights.quality), price: String(weights.price) })
  }, [weights])
  const total = Number(w.on_time) + Number(w.quality) + Number(w.price)
  const save = async (vals: { on_time: number; quality: number; price: number }) => {
    setBusy(true)
    setErr(null)
    try {
      await postJson('/settings', { values: { weight_on_time: vals.on_time, weight_quality: vals.quality, weight_price: vals.price } }, 'PUT')
      onSaved()
    } catch (e) {
      setErr((e as Error).message)
    } finally {
      setBusy(false)
    }
  }
  return (
    <div className="flex flex-wrap items-end gap-3">
      <Input label="Ketepatan kirim (%)" type="number" min={0} max={100} value={w.on_time} onChange={(v) => setW({ ...w, on_time: v })} className="w-36" />
      <Input label="Kualitas (%)" type="number" min={0} max={100} value={w.quality} onChange={(v) => setW({ ...w, quality: v })} className="w-28" />
      <Input label="Harga (%)" type="number" min={0} max={100} value={w.price} onChange={(v) => setW({ ...w, price: v })} className="w-28" />
      <Button variant="primary" disabled={busy || total <= 0} onClick={() => save({ on_time: Number(w.on_time), quality: Number(w.quality), price: Number(w.price) })}>
        {busy ? 'Menyimpan…' : 'Simpan & hitung ulang'}
      </Button>
      <Button variant="ghost" disabled={busy} onClick={() => save({ on_time: 40, quality: 35, price: 25 })}>
        Kembalikan default (40/35/25)
      </Button>
      <span className="text-xs text-muted">Total {num(total)} — bobot dinormalisasi otomatis</span>
      {err && <ErrorBox message={err} />}
    </div>
  )
}

export function scoreColumns(): import('../components/DataTable').Column<VendorScore>[] {
  return [
    { key: 'v', header: 'Vendor', render: (r) => <span className="font-medium text-ink">{r.vendor}</span>, sort: (r) => r.vendor },
    { key: 't', header: 'Skor', align: 'right', render: (r) => <span className="font-semibold">{num(r.total, 1)}</span>, sort: (r) => r.total },
    { key: 'l', header: 'Label', render: (r) => <ScoreLabel label={r.label} />, sort: (r) => r.total },
    { key: 'o', header: 'Tepat waktu', align: 'right', render: (r) => pct(r.on_time_pct), sort: (r) => r.on_time_pct },
    { key: 'late', header: 'PO telat', align: 'right', render: (r) => `${num(r.late)}/${num(r.delivered)}`, sort: (r) => r.late },
    { key: 'q', header: 'Kualitas', align: 'right', render: (r) => (r.avg_quality ? `${num(r.avg_quality, 2)} · rej ${pct(r.reject_pct)}` : '–'), sort: (r) => r.quality_score },
    { key: 'qs', header: 'Skor kualitas', align: 'right', render: (r) => num(r.quality_score, 1), sort: (r) => r.quality_score },
    { key: 'p', header: 'Harga vs pasar', align: 'right', render: (r) => pct(r.price_diff_pct, 1, true), sort: (r) => r.price_diff_pct },
    { key: 'ps', header: 'Skor harga', align: 'right', render: (r) => num(r.price_score, 1), sort: (r) => r.price_score },
    { key: 's', header: 'Spend', align: 'right', render: (r) => rpShort(r.spend), sort: (r) => r.spend },
  ]
}

export default function VendorsPage() {
  const { filterQs, user } = useApp()
  const state = useApi<Data>(`/vendors/scorecard${filterQs}`)
  const nav = useNavigate()
  const canEdit = user.role !== 'viewer'
  return (
    <>
      <PageHeader title="Scorecard vendor" subtitle="Skor 0–100: Baik (≥ 80), Perlu perhatian (60–79), Bermasalah (< 60)." action={<ExportButton view="vendors" />} />
      <FilterBar showVendor={false} />
      <AsyncView state={state} height="h-96">
        {(d) => (
          <div className="flex flex-col gap-4">
            <Card title={`Vendor bermasalah / perlu perhatian (${d.problems.length})`} subtitle="Diurutkan dari skor terendah">
              {d.problems.length === 0 ? (
                <div className="py-4 text-sm text-good">● Semua vendor berlabel Baik pada periode ini.</div>
              ) : (
                <div className="grid grid-cols-1 gap-2 md:grid-cols-2 xl:grid-cols-3">
                  {[...d.problems].sort((a, b) => (a.total ?? 0) - (b.total ?? 0)).map((v) => {
                    const weakest = [
                      { k: 'ketepatan kirim', s: v.on_time_pct },
                      { k: 'kualitas', s: v.quality_score },
                      { k: 'harga', s: v.price_score },
                    ]
                      .filter((x) => x.s !== null)
                      .sort((a, b) => (a.s ?? 0) - (b.s ?? 0))[0]
                    return (
                      <button key={v.vendor_id} onClick={() => nav(`/vendor/${v.vendor_id}`)} className="rounded-xl border border-line p-3 text-left hover:bg-surface-2">
                        <div className="flex items-start justify-between gap-2">
                          <span className="text-sm font-medium text-ink">{v.vendor}</span>
                          <ScoreLabel label={v.label} />
                        </div>
                        <div className="mt-1 font-display text-2xl font-bold text-ink">{num(v.total, 1)}</div>
                        <div className="text-xs text-ink-2">
                          Terlemah: {weakest?.k} ({num(weakest?.s, 1)}) · telat {num(v.late)}/{num(v.delivered)} PO · harga {pct(v.price_diff_pct, 1, true)}
                        </div>
                      </button>
                    )
                  })}
                </div>
              )}
            </Card>
            <Card
              title="Bobot scorecard"
              subtitle={`Saat ini: ketepatan kirim ${num(d.weights.on_time)}% · kualitas ${num(d.weights.quality)}% · harga ${num(d.weights.price)}%`}
            >
              {canEdit ? <WeightEditor weights={d.weights} onSaved={state.reload} /> : <p className="text-xs text-muted">Bobot hanya bisa diubah oleh Data Admin.</p>}
              <p className="mt-2 text-xs text-muted">
                Kualitas = (rata-rata skor − 1) / 4 × 100 − % reject. Harga = 80 − 4 × selisih% vs rata-rata pasar internal untuk item yang sama (sama dengan pasar = 80, 5% lebih murah = 100).
              </p>
            </Card>
            <Card title={`Semua vendor (${d.vendors.length})`} subtitle="Klik vendor untuk detail">
              <DataTable<VendorScore> rows={d.vendors} rowKey={(r) => r.vendor_id} onRowClick={(r) => nav(`/vendor/${r.vendor_id}`)} columns={scoreColumns()} initialSort={{ key: 't', desc: true }} />
            </Card>
            <SplitPoCard qs={filterQs} />
          </div>
        )}
      </AsyncView>
    </>
  )
}
