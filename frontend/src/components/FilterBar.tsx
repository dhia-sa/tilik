import { useEffect, useState } from 'react'
import { useApp } from '../context'
import { Button, Input, Select } from './ui'

function addMonths(d: string, n: number): string {
  const [y, m] = d.split('-').map(Number)
  const t = new Date(Date.UTC(y, m - 1 + n, 1))
  return t.toISOString().slice(0, 10)
}

/** Filter global: rentang tanggal, kategori, vendor. Dipakai bersama semua halaman. */
export function FilterBar({ showVendor = true, showCategory = true }: { showVendor?: boolean; showCategory?: boolean }) {
  const { meta, filter, setFilter } = useApp()
  const [draft, setDraft] = useState(filter)
  useEffect(() => {
    setDraft(filter)
  }, [filter])
  const ref = meta.reference_date
  const monthStart = ref.slice(0, 8) + '01'
  const presets = [
    { label: 'Bulan ini', from: monthStart, to: ref },
    { label: '3 bulan', from: addMonths(monthStart, -2), to: ref },
    { label: '6 bulan', from: addMonths(monthStart, -5), to: ref },
    { label: '12 bulan', from: addMonths(monthStart, -11), to: ref },
    { label: 'Tahun ini', from: ref.slice(0, 4) + '-01-01', to: ref },
  ]
  const dirty = draft.from !== filter.from || draft.to !== filter.to
  const activePreset = presets.find((p) => p.from === filter.from && p.to === filter.to)?.label
  return (
    <div className="mb-4 flex flex-wrap items-end gap-3 rounded-xl border border-line bg-surface px-3 py-2.5">
      <div className="flex flex-col gap-1 text-xs text-ink-2">
        Periode
        <div className="inline-flex flex-wrap rounded-lg border border-line bg-surface-2 p-0.5">
          {presets.map((p) => (
            <button
              key={p.label}
              onClick={() => setFilter({ ...filter, from: p.from, to: p.to })}
              className={`rounded-md px-2 py-1 text-xs font-medium ${activePreset === p.label ? 'bg-surface text-ink shadow-sm' : 'text-ink-2 hover:text-ink'}`}
            >
              {p.label}
            </button>
          ))}
        </div>
      </div>
      <Input label="Dari" type="date" value={draft.from} onChange={(v) => setDraft({ ...draft, from: v })} />
      <Input label="Sampai" type="date" value={draft.to} onChange={(v) => setDraft({ ...draft, to: v })} />
      {dirty && (
        <Button variant="primary" onClick={() => draft.from && draft.to && setFilter({ ...filter, from: draft.from, to: draft.to })}>
          Terapkan
        </Button>
      )}
      {showCategory && (
        <Select label="Kategori" value={filter.category} onChange={(v) => setFilter({ ...filter, category: v })} className="min-w-40">
          <option value="">Semua kategori</option>
          {meta.categories.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </Select>
      )}
      {showVendor && (
        <Select label="Vendor" value={filter.vendor} onChange={(v) => setFilter({ ...filter, vendor: v })} className="min-w-48">
          <option value="">Semua vendor</option>
          {meta.vendors.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </Select>
      )}
      {(filter.category || filter.vendor) && (
        <Button variant="ghost" onClick={() => setFilter({ ...filter, category: '', vendor: '' })}>
          Reset filter
        </Button>
      )}
      <div className="ml-auto text-right text-[11px] leading-tight text-muted">
        Data s/d {meta.data_to ?? '–'}
        <br />
        Dibandingkan periode sebelumnya dengan panjang sama
      </div>
    </div>
  )
}
