import { Link } from 'react-router-dom'
import { dateLabel, num, pct, rp, rpShort } from '../format'
import { useApi } from '../hooks'
import type { Filter, SplitGroup } from '../types'
import { DataTable } from './DataTable'
import { AsyncView, Card } from './ui'

interface Data {
  filter: Filter
  params: { approval_limit: number; min_count: number }
  groups: SplitGroup[]
}

/** Kartu "Kemungkinan split PO" (bonus). `qs` = query string filter, mis. "?from=…&to=…". */
export function SplitPoCard({ qs, showVendor = true }: { qs: string; showVendor?: boolean }) {
  const state = useApi<Data>(`/vendors/split-po${qs}`)
  return (
    <AsyncView state={state} height="h-40">
      {(d) => (
        <Card
          title={`Kemungkinan split PO (${d.groups.length})`}
          subtitle={`≥ ${num(d.params.min_count)} PO ke vendor yang sama di hari yang sama, setiap PO di bawah batas approval ${rp(d.params.approval_limit)}, tetapi totalnya melewati batas. Ini indikasi untuk ditinjau, bukan kesimpulan. Batas dan jumlah minimum bisa diubah di Pengaturan.`}
        >
          {d.groups.length === 0 ? (
            <div className="py-3 text-sm text-good">● Tidak ada pola split PO pada periode ini.</div>
          ) : (
            <DataTable<SplitGroup>
              rows={d.groups}
              rowKey={(g) => `${g.vendor_id}-${g.po_date}`}
              maxHeight="420px"
              initialSort={{ key: 'd', desc: true }}
              columns={[
                { key: 'd', header: 'Tanggal', render: (g) => dateLabel(g.po_date), sort: (g) => g.po_date },
                ...(showVendor
                  ? [{ key: 'v', header: 'Vendor', render: (g: SplitGroup) => <Link className="text-ink hover:underline" to={`/vendor/${g.vendor_id}`}>{g.vendor}</Link>, sort: (g: SplitGroup) => g.vendor }]
                  : []),
                { key: 'n', header: 'Jumlah PO', align: 'right', render: (g) => num(g.po_count), sort: (g) => g.po_count },
                { key: 't', header: 'Total', align: 'right', render: (g) => <span className="font-medium">{rpShort(g.total)}</span>, sort: (g) => g.total },
                { key: 'm', header: 'PO terbesar', align: 'right', render: (g) => rpShort(g.largest_po), sort: (g) => g.largest_po },
                { key: 'o', header: 'Di atas batas', align: 'right', render: (g) => pct(g.over_limit_pct, 1, true), sort: (g) => g.over_limit_pct },
                {
                  key: 'i',
                  header: 'Item berulang',
                  render: (g) => (g.repeated_items.length ? g.repeated_items.join(', ') : <span className="text-muted">–</span>),
                  sort: (g) => g.repeated_items.length,
                },
                {
                  key: 'p',
                  header: 'Nomor PO',
                  render: (g) => (
                    <div className="flex flex-col text-xs text-ink-2">
                      {g.pos.map((p) => (
                        <span key={p.po_no}>
                          {p.po_no} · {rpShort(p.amount)}
                        </span>
                      ))}
                    </div>
                  ),
                },
              ]}
            />
          )}
        </Card>
      )}
    </AsyncView>
  )
}
