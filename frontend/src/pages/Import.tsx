import { useState } from 'react'
import { api } from '../api'
import { DataTable } from '../components/DataTable'
import { AsyncView, Button, Card, ErrorBox, PageHeader, Select, Spinner, Tabs } from '../components/ui'
import { useApp } from '../context'
import { num, rp } from '../format'
import { useApi } from '../hooks'

interface RowError { line: number; raw: string[]; reasons: string[] }
interface Preview {
  token: string
  kind: 'po' | 'vendor_rating'
  file_name: string
  rows_ok: number
  rows_error: number
  columns: string[]
  valid_sample: Record<string, unknown>[]
  errors: RowError[]
  new_vendors: string[]
  duration_ms: number
}
interface Batch {
  id: number; kind: string; file_name: string; rows_ok: number; rows_error: number; user: string | null
  created_at: string; rolled_back: boolean; rolled_back_at: string | null; duration_ms: number
}
interface CommitResult { batch_id: number; rows_ok: number; rows_error: number; duration_ms: number; new_vendors: number; new_items: number }

export default function ImportPage() {
  const { reloadMeta } = useApp()
  const [kind, setKind] = useState<'po' | 'vendor_rating'>('po')
  const [file, setFile] = useState<File | null>(null)
  const [preview, setPreview] = useState<Preview | null>(null)
  const [tab, setTab] = useState<'error' | 'valid'>('error')
  const [busy, setBusy] = useState<string | null>(null)
  const [err, setErr] = useState<string | null>(null)
  const [done, setDone] = useState<string | null>(null)
  const batches = useApi<{ batches: Batch[] }>('/imports')

  async function doPreview() {
    if (!file) return
    setBusy('preview'); setErr(null); setDone(null); setPreview(null)
    const fd = new FormData()
    fd.append('kind', kind)
    fd.append('file', file)
    try {
      const p = await api<Preview>('/imports/preview', { method: 'POST', body: fd })
      setPreview(p)
      setTab(p.rows_error > 0 ? 'error' : 'valid')
    } catch (e) {
      setErr((e as Error).message)
    } finally {
      setBusy(null)
    }
  }

  async function commit() {
    if (!preview) return
    setBusy('commit'); setErr(null)
    try {
      const r = await api<CommitResult>(`/imports/${preview.token}/commit`, { method: 'POST' })
      setDone(`Batch #${r.batch_id} tersimpan: ${num(r.rows_ok)} baris valid (${num(r.rows_error)} baris error dilewati) dalam ${num(r.duration_ms / 1000, 2)} detik. Agregat & alert sudah dihitung ulang.`)
      setPreview(null)
      setFile(null)
      batches.reload()
      reloadMeta()
    } catch (e) {
      setErr((e as Error).message)
    } finally {
      setBusy(null)
    }
  }

  async function rollback(b: Batch) {
    if (!window.confirm(`Rollback batch #${b.id} (${b.file_name})? Semua ${num(b.rows_ok)} baris dari batch ini akan dihapus.`)) return
    setBusy(`rb${b.id}`); setErr(null); setDone(null)
    try {
      const r = await api<{ deleted_po_lines: number; deleted_ratings: number }>(`/imports/${b.id}/rollback`, { method: 'POST' })
      setDone(`Batch #${b.id} di-rollback: ${num(r.deleted_po_lines)} baris PO dan ${num(r.deleted_ratings)} penilaian dihapus. Agregat dihitung ulang.`)
      batches.reload()
      reloadMeta()
    } catch (e) {
      setErr((e as Error).message)
    } finally {
      setBusy(null)
    }
  }

  const poCols = ['po_no', 'po_date', 'vendor', 'item', 'qty', 'unit', 'unit_price', 'base_qty', 'base_unit', 'base_unit_price']
  const ratingCols = ['period', 'vendor', 'quality_score', 'reject_qty', 'total_qty', 'feedback']

  return (
    <>
      <PageHeader title="Import data" subtitle="Upload CSV → preview & validasi per baris → konfirmasi. Setiap import tercatat sebagai batch yang bisa di-rollback." />
      <div className="flex flex-col gap-4">
        <Card title="1. Pilih file">
          <div className="flex flex-wrap items-end gap-3">
            <Select label="Jenis data" value={kind} onChange={(v) => { setKind(v as 'po' | 'vendor_rating'); setPreview(null) }} className="w-56">
              <option value="po">CSV PO</option>
              <option value="vendor_rating">CSV penilaian vendor</option>
            </Select>
            <label className="flex flex-col gap-1 text-xs text-ink-2">
              File CSV
              <input
                type="file"
                accept=".csv,text/csv"
                onChange={(e) => { setFile(e.target.files?.[0] ?? null); setPreview(null); setDone(null) }}
                className="rounded-lg border border-line bg-surface px-2 py-1 text-sm text-ink file:mr-2 file:rounded file:border-0 file:bg-surface-2 file:px-2 file:py-1 file:text-ink"
              />
            </label>
            <Button variant="primary" onClick={doPreview} disabled={!file || busy !== null}>
              {busy === 'preview' ? <><Spinner /> Memvalidasi…</> : 'Preview & validasi'}
            </Button>
          </div>
          <p className="mt-2 text-xs text-muted">
            {kind === 'po'
              ? 'Kolom: po_no, po_date, vendor, category, item, commodity (opsional), qty, unit, unit_price, delivered_date (opsional), promised_date. Tanggal YYYY-MM-DD, angka tanpa pemisah ribuan (1234.5).'
              : 'Kolom: period (YYYY-MM), vendor, quality_score (1–5), reject_qty, total_qty, feedback.'}{' '}
            Contoh file ada di folder <code>seed/</code> (po_12bulan.csv, po_kotor.csv, penilaian_vendor_12bulan.csv).
          </p>
        </Card>

        {err && <ErrorBox message={err} />}
        {done && <div className="rounded-lg border border-line bg-surface-2 px-4 py-3 text-sm text-good">● {done}</div>}

        {preview && (
          <Card
            title={`2. Hasil validasi · ${preview.file_name}`}
            subtitle={`Divalidasi dalam ${num(preview.duration_ms)} ms. Data belum disimpan.`}
            action={
              <div className="flex gap-2">
                <Button onClick={() => setPreview(null)}>Batal</Button>
                <Button variant="primary" onClick={commit} disabled={preview.rows_ok === 0 || busy !== null}>
                  {busy === 'commit' ? <><Spinner /> Menyimpan…</> : `Lanjutkan import ${num(preview.rows_ok)} baris valid`}
                </Button>
              </div>
            }
          >
            <div className="mb-3 grid grid-cols-2 gap-3 md:grid-cols-4">
              <div className="rounded-lg border border-line p-3"><div className="text-xs text-ink-2">Baris valid</div><div className="text-xl font-semibold text-good">{num(preview.rows_ok)}</div></div>
              <div className="rounded-lg border border-line p-3"><div className="text-xs text-ink-2">Baris error</div><div className={`text-xl font-semibold ${preview.rows_error ? 'text-crit' : 'text-ink'}`}>{num(preview.rows_error)}</div></div>
              <div className="col-span-2 rounded-lg border border-line p-3 text-xs text-ink-2">
                Vendor baru yang akan dibuat: {preview.new_vendors.length ? preview.new_vendors.join(', ') : 'tidak ada'}
                {preview.rows_error > 0 && <div className="mt-1">Baris error tidak akan disimpan. Perbaiki di file lalu import ulang bila perlu.</div>}
              </div>
            </div>
            <Tabs value={tab} onChange={setTab} options={[{ value: 'error', label: `Baris error (${preview.rows_error})` }, { value: 'valid', label: `Baris valid (${preview.rows_ok})` }]} />
            <div className="mt-3">
              {tab === 'error' ? (
                <DataTable<RowError>
                  rows={preview.errors}
                  rowKey={(r) => r.line}
                  maxHeight="420px"
                  empty={<div className="py-6 text-center text-sm text-good">● Tidak ada baris error</div>}
                  columns={[
                    { key: 'l', header: 'Baris', render: (r) => num(r.line), sort: (r) => r.line },
                    { key: 'r', header: 'Alasan', render: (r) => <ul className="list-disc pl-4 text-crit">{r.reasons.map((x) => <li key={x}>{x}</li>)}</ul> },
                    { key: 'd', header: 'Isi baris', render: (r) => <span className="font-mono text-[11px] text-ink-2">{r.raw.join(' | ')}</span> },
                  ]}
                />
              ) : (
                <div className="max-h-[420px] overflow-auto">
                  <table className="w-full text-xs">
                    <thead className="sticky top-0 bg-surface">
                      <tr>{(preview.kind === 'po' ? poCols : ratingCols).map((c) => <th key={c} className="px-2 py-1.5 text-left font-medium text-ink-2">{c}</th>)}</tr>
                    </thead>
                    <tbody>
                      {preview.valid_sample.map((r, i) => (
                        <tr key={i} className="border-t border-line">
                          {(preview.kind === 'po' ? poCols : ratingCols).map((c) => {
                            const v = r[c]
                            return <td key={c} className="px-2 py-1">{c === 'base_unit_price' || c === 'unit_price' ? rp(Number(v)) : typeof v === 'number' ? num(v, 2) : String(v ?? '–')}</td>
                          })}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                  {preview.rows_ok > preview.valid_sample.length && <div className="py-2 text-xs text-muted">Menampilkan {preview.valid_sample.length} dari {num(preview.rows_ok)} baris valid.</div>}
                </div>
              )}
            </div>
          </Card>
        )}

        <Card title="Riwayat import" subtitle="Rollback menghapus semua baris dari batch tersebut lalu menghitung ulang agregat & alert">
          <AsyncView state={batches}>
            {(d) => (
              <DataTable<Batch>
                rows={d.batches}
                rowKey={(b) => b.id}
                rowClass={(b) => (b.rolled_back ? 'opacity-50' : '')}
                columns={[
                  { key: 'id', header: '#', render: (b) => b.id },
                  { key: 'k', header: 'Jenis', render: (b) => (b.kind === 'po' ? 'PO' : 'Penilaian vendor') },
                  { key: 'f', header: 'File', render: (b) => b.file_name },
                  { key: 'ok', header: 'Valid', align: 'right', render: (b) => num(b.rows_ok) },
                  { key: 'er', header: 'Error', align: 'right', render: (b) => num(b.rows_error) },
                  { key: 'ms', header: 'Durasi', align: 'right', render: (b) => `${num(b.duration_ms / 1000, 2)} dtk` },
                  { key: 'u', header: 'Oleh', render: (b) => b.user ?? '–' },
                  { key: 't', header: 'Waktu (UTC)', render: (b) => b.created_at },
                  {
                    key: 'a',
                    header: '',
                    render: (b) =>
                      b.rolled_back ? (
                        <span className="text-xs text-muted">di-rollback {b.rolled_back_at}</span>
                      ) : (
                        <Button variant="danger" onClick={() => rollback(b)} disabled={busy !== null}>
                          {busy === `rb${b.id}` ? 'Memproses…' : 'Rollback'}
                        </Button>
                      ),
                  },
                ]}
              />
            )}
          </AsyncView>
        </Card>
      </div>
    </>
  )
}
