import { useMemo, useState, type ReactNode } from 'react'

export interface Column<T> {
  key: string
  header: string
  render: (row: T) => ReactNode
  sort?: (row: T) => number | string | null
  align?: 'left' | 'right'
  className?: string
}

/** Tabel sederhana dengan sorting per kolom. */
export function DataTable<T>(props: {
  rows: T[]
  columns: Column<T>[]
  rowKey: (row: T, i: number) => string | number
  onRowClick?: (row: T) => void
  initialSort?: { key: string; desc: boolean }
  empty?: ReactNode
  maxHeight?: string
  rowClass?: (row: T) => string
}) {
  const [sort, setSort] = useState(props.initialSort ?? null)
  const rows = useMemo(() => {
    if (!sort) return props.rows
    const col = props.columns.find((c) => c.key === sort.key)
    if (!col?.sort) return props.rows
    const get = col.sort
    return [...props.rows].sort((a, b) => {
      const x = get(a)
      const y = get(b)
      if (x === y) return 0
      if (x === null) return 1
      if (y === null) return -1
      const r = x < y ? -1 : 1
      return sort.desc ? -r : r
    })
  }, [props.rows, props.columns, sort])
  if (props.rows.length === 0) return <>{props.empty ?? <div className="py-8 text-center text-sm text-muted">Tidak ada data</div>}</>
  return (
    <div className="overflow-auto" style={{ maxHeight: props.maxHeight }}>
      <table className="w-full border-collapse text-sm">
        <thead className="sticky top-0 z-[1] bg-surface">
          <tr className="border-b border-line">
            {props.columns.map((c) => (
              <th
                key={c.key}
                className={`whitespace-nowrap px-3 py-2.5 text-xs font-medium text-ink-2 ${c.align === 'right' ? 'text-right' : 'text-left'} ${c.sort ? 'cursor-pointer select-none hover:text-ink' : ''}`}
                onClick={() => c.sort && setSort((s) => ({ key: c.key, desc: s?.key === c.key ? !s.desc : true }))}
              >
                {c.header}
                {sort?.key === c.key && <span aria-hidden> {sort.desc ? '↓' : '↑'}</span>}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr
              key={props.rowKey(r, i)}
              onClick={() => props.onRowClick?.(r)}
              className={`border-b border-line/70 transition-colors duration-200 last:border-0 hover:bg-bg ${props.onRowClick ? 'cursor-pointer' : ''} ${props.rowClass?.(r) ?? ''}`}
            >
              {props.columns.map((c) => (
                <td key={c.key} className={`px-3 py-2.5 ${c.align === 'right' ? 'text-right tabular-nums' : ''} ${c.className ?? ''}`}>
                  {c.render(r)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
