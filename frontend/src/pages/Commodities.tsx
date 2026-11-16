import { useNavigate } from 'react-router-dom'
import { Sparkline } from '../components/charts'
import { DataTable } from '../components/DataTable'
import { ExportButton } from '../components/Export'
import { AlertKind, AsyncView, Card, PageHeader } from '../components/ui'
import { dateLabel, num, pct, rp } from '../format'
import { useApi } from '../hooks'
import type { AlertRow, CommodityRow } from '../types'

interface Data {
  reference_date: string
  commodities: CommodityRow[]
  alerts: AlertRow[]
  params: { threshold_pct: number; sigma: number; min_dev_pct: number }
}

export default function CommoditiesPage() {
  const state = useApi<Data>('/commodities')
  const nav = useNavigate()
  return (
    <>
      <PageHeader title="Harga komoditas mingguan" subtitle="Rata-rata tertimbang qty per satuan dasar · minggu ISO Senin–Minggu (WIB)" action={<ExportButton view="commodities" />} />
      <AsyncView state={state}>
        {(d) => (
          <div className="flex flex-col gap-4">
            <Card
              title={`Minggu ${d.commodities[0]?.this_week ?? ''}`}
              subtitle={`Data s/d ${dateLabel(d.reference_date)} · alert jika naik > ${num(d.params.threshold_pct, 1)}% vs minggu lalu, atau > rata-rata 12 minggu + ${num(d.params.sigma, 1)}σ`}
            >
              <DataTable<CommodityRow>
                rows={d.commodities}
                rowKey={(r) => r.id}
                onRowClick={(r) => nav(`/komoditas/${r.id}`)}
                columns={[
                  {
                    key: 'n',
                    header: 'Komoditas',
                    render: (r) => (
                      <span className="flex items-center gap-2 font-medium text-ink">
                        {r.name}
                        {r.alert && <AlertKind kind="price" />}
                      </span>
                    ),
                    sort: (r) => r.name,
                  },
                  { key: 'tw', header: 'Minggu ini', align: 'right', render: (r) => (r.price_this_week ? `${rp(r.price_this_week)}/${r.base_unit}` : '–'), sort: (r) => r.price_this_week },
                  { key: 'lw', header: 'Minggu lalu', align: 'right', render: (r) => (r.price_last_week ? `${rp(r.price_last_week)}/${r.base_unit}` : '–'), sort: (r) => r.price_last_week },
                  {
                    key: 'c',
                    header: 'Δ%',
                    align: 'right',
                    render: (r) => (
                      <span className={r.change_pct !== null && r.change_pct > d.params.threshold_pct ? 'font-semibold text-crit' : r.change_pct !== null && r.change_pct > 0 ? 'text-ink' : 'text-ink-2'}>
                        {r.change_pct !== null && r.change_pct > 0 ? '▲ ' : r.change_pct !== null && r.change_pct < 0 ? '▼ ' : ''}
                        {pct(r.change_pct, 1, true)}
                      </span>
                    ),
                    sort: (r) => r.change_pct,
                  },
                  { key: 'q', header: 'Qty minggu ini', align: 'right', render: (r) => `${num(r.qty_this_week)} ${r.base_unit}`, sort: (r) => r.qty_this_week },
                  { key: 's', header: 'Tren 12 minggu', render: (r) => <Sparkline points={r.sparkline} alert={r.alert} /> },
                ]}
              />
            </Card>
            <Card title="Riwayat alert harga" subtitle="Klik untuk membuka detail komoditas">
              <DataTable<AlertRow>
                rows={d.alerts}
                rowKey={(r) => r.id}
                maxHeight="360px"
                onRowClick={(r) => nav(`/komoditas/${r.ref_id}`)}
                columns={[
                  { key: 'w', header: 'Minggu', render: (r) => r.week, sort: (r) => r.period_start },
                  { key: 'c', header: 'Komoditas', render: (r) => r.ref_name, sort: (r) => r.ref_name },
                  { key: 'm', header: 'Keterangan', render: (r) => <span className="text-ink-2">{r.message.split('— ')[1] ?? r.message}</span> },
                ]}
              />
            </Card>
          </div>
        )}
      </AsyncView>
    </>
  )
}
