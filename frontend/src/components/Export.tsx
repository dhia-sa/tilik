import { useApp } from '../context'

/** Tombol export Excel sesuai filter aktif. */
export function ExportButton({ view, extraQs = '' }: { view: 'spend' | 'commodities' | 'vendors'; extraQs?: string }) {
  const { filterQs } = useApp()
  const q = filterQs ? filterQs + (extraQs ? `&${extraQs}` : '') : extraQs ? `?${extraQs}` : ''
  return (
    <a
      href={`/api/export/${view}.xlsx${q}`}
      className="inline-flex items-center gap-1.5 rounded-lg border border-line bg-surface px-3 py-1.5 text-sm font-medium text-ink hover:bg-surface-2"
    >
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden>
        <path d="M12 4v11m0 0l-4-4m4 4l4-4M5 20h14" />
      </svg>
      Export Excel
    </a>
  )
}
