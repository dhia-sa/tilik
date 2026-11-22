import { Download } from 'lucide-react'
import { useApp } from '../context'
import { DEMO, DEMO_READONLY_MSG } from '../demo'

/** Tombol export Excel sesuai filter aktif. */
export function ExportButton({ view, extraQs = '' }: { view: 'spend' | 'commodities' | 'vendors'; extraQs?: string }) {
  const { filterQs } = useApp()
  if (DEMO)
    return (
      <span title={DEMO_READONLY_MSG} className="inline-flex h-10 cursor-not-allowed items-center gap-2 rounded-xl border border-line bg-surface px-4 text-sm font-medium text-muted">
        <Download size={18} strokeWidth={1.75} aria-hidden />
        Export Excel
      </span>
    )
  const q = filterQs ? filterQs + (extraQs ? `&${extraQs}` : '') : extraQs ? `?${extraQs}` : ''
  return (
    <a
      href={`/api/export/${view}.xlsx${q}`}
      className="inline-flex h-10 items-center gap-2 rounded-xl border border-line bg-surface px-4 text-sm font-medium text-ink hover:border-line-strong hover:bg-surface-2"
    >
      <Download size={18} strokeWidth={1.75} aria-hidden />
      Export Excel
    </a>
  )
}
