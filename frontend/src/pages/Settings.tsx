import { useEffect, useState } from 'react'
import { api, postJson } from '../api'
import { DataTable } from '../components/DataTable'
import { AsyncView, Button, Card, ErrorBox, Input, PageHeader, Select, Tabs } from '../components/ui'
import { useApp } from '../context'
import { num } from '../format'
import { useApi } from '../hooks'

interface SettingsData {
  settings: Record<string, string>
  categories: { id: number; name: string; items: number }[]
  commodities: { id: number; name: string; base_unit: string; items: number }[]
  items: { id: number; name: string; category_id: number; category: string; commodity_id: number | null; commodity: string | null; base_unit: string; po_lines: number }[]
  units: { id: number; item_id: number | null; item: string | null; from_unit: string; base_unit: string; to_base: number }[]
  users: { id: number; email: string; name: string; role: string; created_at: string }[] | null
}

const LABELS: Record<string, string> = {
  price_alert_threshold_pct: 'Alert harga: kenaikan minggu ini vs minggu lalu (%)',
  price_alert_sigma: 'Alert harga: k × standar deviasi (12 minggu)',
  price_alert_min_dev_pct: 'Alert harga: minimal di atas rata-rata 12 minggu (%)',
  vendor_alert_on_time_pct: 'Alert vendor: ketepatan kirim bulanan di bawah (%)',
  vendor_alert_reject_pct: 'Alert vendor: reject bulanan di atas (%)',
  weight_on_time: 'Bobot scorecard: ketepatan kirim',
  weight_quality: 'Bobot scorecard: kualitas',
  weight_price: 'Bobot scorecard: harga',
}

type Tab = 'threshold' | 'master' | 'items' | 'units' | 'users'

function useAction(onDone: () => void) {
  const [err, setErr] = useState<string | null>(null)
  const [msg, setMsg] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const run = async (fn: () => Promise<unknown>, ok: string) => {
    setBusy(true); setErr(null); setMsg(null)
    try {
      await fn()
      setMsg(ok)
      onDone()
    } catch (e) {
      setErr((e as Error).message)
    } finally {
      setBusy(false)
    }
  }
  return { err, msg, busy, run }
}

function Feedback({ err, msg }: { err: string | null; msg: string | null }) {
  if (err) return <ErrorBox message={err} />
  if (msg) return <div className="text-sm text-good">● {msg}</div>
  return null
}

function ThresholdTab({ d, reload }: { d: SettingsData; reload: () => void }) {
  const [vals, setVals] = useState<Record<string, string>>(d.settings)
  useEffect(() => {
    setVals(d.settings)
  }, [d.settings])
  const a = useAction(reload)
  return (
    <Card title="Threshold alert & bobot scorecard" subtitle="Setelah disimpan, alert dan skor dihitung ulang otomatis.">
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
        {Object.keys(LABELS).map((k) => (
          <Input key={k} label={LABELS[k]} type="number" step={0.5} value={vals[k] ?? ''} onChange={(v) => setVals({ ...vals, [k]: v })} />
        ))}
      </div>
      <div className="mt-3 flex items-center gap-3">
        <Button
          variant="primary"
          disabled={a.busy}
          onClick={() => a.run(() => postJson('/settings', { values: Object.fromEntries(Object.keys(LABELS).map((k) => [k, Number(vals[k])])) }, 'PUT'), 'Pengaturan disimpan, alert & skor dihitung ulang')}
        >
          Simpan
        </Button>
        <Feedback err={a.err} msg={a.msg} />
      </div>
    </Card>
  )
}

function MasterTab({ d, reload }: { d: SettingsData; reload: () => void }) {
  const [cat, setCat] = useState('')
  const [com, setCom] = useState('')
  const [comUnit, setComUnit] = useState('kg')
  const a = useAction(reload)
  return (
    <div className="grid grid-cols-1 gap-4 xl:grid-cols-2">
      <Card title="Kategori">
        <div className="mb-3 flex items-end gap-2">
          <Input label="Kategori baru" value={cat} onChange={setCat} />
          <Button variant="primary" disabled={!cat || a.busy} onClick={() => a.run(() => postJson('/master/categories', { name: cat }).then(() => setCat('')), 'Kategori ditambahkan')}>Tambah</Button>
        </div>
        <DataTable
          rows={d.categories}
          rowKey={(r) => r.id}
          columns={[
            { key: 'n', header: 'Nama', render: (r) => r.name },
            { key: 'i', header: 'Item', align: 'right', render: (r) => num(r.items) },
            { key: 'x', header: '', render: (r) => <Button variant="ghost" disabled={r.items > 0 || a.busy} onClick={() => a.run(() => api(`/master/categories/${r.id}`, { method: 'DELETE' }), 'Kategori dihapus')}>Hapus</Button> },
          ]}
        />
      </Card>
      <Card title="Komoditas">
        <div className="mb-3 flex items-end gap-2">
          <Input label="Komoditas baru" value={com} onChange={setCom} />
          <Select label="Satuan dasar" value={comUnit} onChange={setComUnit}>
            <option value="kg">kg</option><option value="liter">liter</option><option value="pcs">pcs</option>
          </Select>
          <Button variant="primary" disabled={!com || a.busy} onClick={() => a.run(() => postJson('/master/commodities', { name: com, base_unit: comUnit }).then(() => setCom('')), 'Komoditas ditambahkan')}>Tambah</Button>
        </div>
        <DataTable
          rows={d.commodities}
          rowKey={(r) => r.id}
          columns={[
            { key: 'n', header: 'Nama', render: (r) => r.name },
            { key: 'u', header: 'Satuan dasar', render: (r) => r.base_unit },
            { key: 'i', header: 'Item', align: 'right', render: (r) => num(r.items) },
            { key: 'x', header: '', render: (r) => <Button variant="ghost" disabled={r.items > 0 || a.busy} onClick={() => a.run(() => api(`/master/commodities/${r.id}`, { method: 'DELETE' }), 'Komoditas dihapus')}>Hapus</Button> },
          ]}
        />
      </Card>
      <div className="xl:col-span-2"><Feedback err={a.err} msg={a.msg} /></div>
    </div>
  )
}

function ItemsTab({ d, reload }: { d: SettingsData; reload: () => void }) {
  const a = useAction(reload)
  const [q, setQ] = useState('')
  const rows = d.items.filter((i) => i.name.toLowerCase().includes(q.toLowerCase()))
  return (
    <Card title="Mapping item → kategori & komoditas" subtitle="Item komoditas dipakai untuk harga mingguan. Satuan dasar item harus sama dengan komoditasnya.">
      <div className="mb-3 flex items-center gap-3">
        <Input placeholder="Cari item…" value={q} onChange={setQ} />
        <Feedback err={a.err} msg={a.msg} />
      </div>
      <DataTable
        rows={rows}
        rowKey={(r) => r.id}
        maxHeight="560px"
        columns={[
          { key: 'n', header: 'Item', render: (r) => r.name, sort: (r) => r.name },
          { key: 'u', header: 'Satuan dasar', render: (r) => r.base_unit },
          {
            key: 'c',
            header: 'Kategori',
            render: (r) => (
              <select className="rounded border border-line bg-surface px-1 py-0.5 text-sm" value={r.category_id} disabled={a.busy}
                onChange={(e) => a.run(() => postJson(`/master/items/${r.id}`, { category_id: Number(e.target.value), commodity_id: r.commodity_id }, 'PUT'), `Kategori ${r.name} diperbarui`)}>
                {d.categories.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
              </select>
            ),
            sort: (r) => r.category,
          },
          {
            key: 'm',
            header: 'Komoditas',
            render: (r) => (
              <select className="rounded border border-line bg-surface px-1 py-0.5 text-sm" value={r.commodity_id ?? ''} disabled={a.busy}
                onChange={(e) => a.run(() => postJson(`/master/items/${r.id}`, { category_id: r.category_id, commodity_id: e.target.value ? Number(e.target.value) : null }, 'PUT'), `Komoditas ${r.name} diperbarui`)}>
                <option value="">— bukan komoditas —</option>
                {d.commodities.map((c) => <option key={c.id} value={c.id}>{c.name} ({c.base_unit})</option>)}
              </select>
            ),
            sort: (r) => r.commodity ?? '',
          },
          { key: 'p', header: 'Baris PO', align: 'right', render: (r) => num(r.po_lines), sort: (r) => r.po_lines },
        ]}
      />
    </Card>
  )
}

function UnitsTab({ d, reload }: { d: SettingsData; reload: () => void }) {
  const a = useAction(reload)
  const [f, setF] = useState({ item_id: '', from_unit: '', base_unit: 'kg', to_base: '' })
  return (
    <Card title="Konversi satuan" subtitle="Konversi global berlaku untuk semua item dengan satuan dasar yang sama; konversi item (mis. karton) hanya untuk item itu. PO dinormalisasi ulang setelah perubahan.">
      <div className="mb-3 flex flex-wrap items-end gap-2">
        <Select label="Berlaku untuk" value={f.item_id} onChange={(v) => setF({ ...f, item_id: v, base_unit: d.items.find((i) => String(i.id) === v)?.base_unit ?? f.base_unit })} className="w-64">
          <option value="">Global (semua item)</option>
          {d.items.map((i) => <option key={i.id} value={i.id}>{i.name}</option>)}
        </Select>
        <Input label="Satuan" placeholder="mis. sak 10 kg" value={f.from_unit} onChange={(v) => setF({ ...f, from_unit: v })} />
        <Input label="= berapa satuan dasar" type="number" value={f.to_base} onChange={(v) => setF({ ...f, to_base: v })} className="w-36" />
        <Select label="Satuan dasar" value={f.base_unit} onChange={(v) => setF({ ...f, base_unit: v })}>
          <option value="kg">kg</option><option value="liter">liter</option><option value="pcs">pcs</option>
        </Select>
        <Button variant="primary" disabled={!f.from_unit || !f.to_base || a.busy}
          onClick={() => a.run(() => postJson('/master/units', { item_id: f.item_id ? Number(f.item_id) : null, from_unit: f.from_unit, base_unit: f.base_unit, to_base: Number(f.to_base) }).then(() => setF({ ...f, from_unit: '', to_base: '' })), 'Konversi ditambahkan')}>
          Tambah
        </Button>
        <Feedback err={a.err} msg={a.msg} />
      </div>
      <DataTable
        rows={d.units}
        rowKey={(r) => r.id}
        maxHeight="480px"
        columns={[
          { key: 'i', header: 'Berlaku untuk', render: (r) => r.item ?? <span className="text-muted">Global</span> },
          { key: 'f', header: 'Satuan', render: (r) => r.from_unit },
          { key: 'b', header: 'Konversi', render: (r) => `1 ${r.from_unit} = ${num(r.to_base, 3)} ${r.base_unit}` },
          { key: 'x', header: '', render: (r) => <Button variant="ghost" disabled={a.busy} onClick={() => a.run(() => api(`/master/units/${r.id}`, { method: 'DELETE' }), 'Konversi dihapus')}>Hapus</Button> },
        ]}
      />
    </Card>
  )
}

function UsersTab({ d, reload }: { d: SettingsData; reload: () => void }) {
  const { user } = useApp()
  const a = useAction(reload)
  const [f, setF] = useState({ email: '', name: '', role: 'viewer', password: '' })
  if (!d.users) return <ErrorBox message="Kelola user hanya untuk Admin IT." />
  return (
    <Card title="User & role" subtitle="Viewer: dashboard, Tanya Data, export · Data Admin: + import, master data, threshold · Admin IT: + kelola user">
      <div className="mb-3 flex flex-wrap items-end gap-2">
        <Input label="Email" value={f.email} onChange={(v) => setF({ ...f, email: v })} />
        <Input label="Nama" value={f.name} onChange={(v) => setF({ ...f, name: v })} />
        <Select label="Role" value={f.role} onChange={(v) => setF({ ...f, role: v })}>
          <option value="viewer">Viewer</option><option value="data_admin">Data Admin</option><option value="admin">Admin IT</option>
        </Select>
        <Input label="Password (min. 8)" type="password" value={f.password} onChange={(v) => setF({ ...f, password: v })} />
        <Button variant="primary" disabled={a.busy} onClick={() => a.run(() => postJson('/users', f).then(() => setF({ email: '', name: '', role: 'viewer', password: '' })), 'User ditambahkan')}>Tambah user</Button>
        <Feedback err={a.err} msg={a.msg} />
      </div>
      <DataTable
        rows={d.users}
        rowKey={(r) => r.id}
        columns={[
          { key: 'e', header: 'Email', render: (r) => r.email },
          { key: 'n', header: 'Nama', render: (r) => r.name },
          {
            key: 'r',
            header: 'Role',
            render: (r) => (
              <select className="rounded border border-line bg-surface px-1 py-0.5 text-sm" value={r.role} disabled={a.busy || r.id === user.id}
                onChange={(e) => a.run(() => postJson(`/users/${r.id}`, { role: e.target.value }, 'PUT'), `Role ${r.email} diperbarui`)}>
                <option value="viewer">Viewer</option><option value="data_admin">Data Admin</option><option value="admin">Admin IT</option>
              </select>
            ),
          },
          { key: 't', header: 'Dibuat', render: (r) => r.created_at },
          {
            key: 'x',
            header: '',
            render: (r) =>
              r.id === user.id ? <span className="text-xs text-muted">akun Anda</span> : (
                <Button variant="ghost" disabled={a.busy} onClick={() => window.confirm(`Hapus user ${r.email}?`) && a.run(() => api(`/users/${r.id}`, { method: 'DELETE' }), 'User dihapus')}>Hapus</Button>
              ),
          },
        ]}
      />
    </Card>
  )
}

export default function SettingsPage() {
  const { user, reloadMeta } = useApp()
  const state = useApi<SettingsData>('/settings')
  const [tab, setTab] = useState<Tab>('threshold')
  const reload = () => {
    state.reload()
    reloadMeta()
  }
  const options: { value: Tab; label: string }[] = [
    { value: 'threshold', label: 'Threshold & bobot' },
    { value: 'master', label: 'Kategori & komoditas' },
    { value: 'items', label: 'Mapping item' },
    { value: 'units', label: 'Satuan' },
    ...(user.role === 'admin' ? [{ value: 'users' as Tab, label: 'User & role' }] : []),
  ]
  return (
    <>
      <PageHeader title="Pengaturan" subtitle="Master data, threshold alert, bobot scorecard, dan user." action={<Tabs value={tab} onChange={setTab} options={options} />} />
      <AsyncView state={state}>
        {(d) =>
          tab === 'threshold' ? <ThresholdTab d={d} reload={reload} />
          : tab === 'master' ? <MasterTab d={d} reload={reload} />
          : tab === 'items' ? <ItemsTab d={d} reload={reload} />
          : tab === 'units' ? <UnitsTab d={d} reload={reload} />
          : <UsersTab d={d} reload={reload} />
        }
      </AsyncView>
    </>
  )
}
