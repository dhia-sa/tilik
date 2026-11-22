import { Download } from 'lucide-react'
import { useApp } from '../context'

/** Tombol export Excel sesuai filter aktif. */
export function ExportButton({ view, extraQs = '' }: { view: 'spend' | 'commodities' | 'vendors'; extraQs?: string }) {
  const { filterQs } = useApp()
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
