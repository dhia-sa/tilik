import type { ReactNode } from 'react'
import { Line, LineChart, ResponsiveContainer, YAxis } from 'recharts'
import type { WeekPoint } from '../types'

export const AXIS = { stroke: 'var(--grid)', tickLine: false, axisLine: false } as const
export const GRID = { stroke: 'var(--grid)', vertical: false } as const

/** Tooltip gaya kartu, teks memakai warna teks (bukan warna seri). */
export function TooltipBox(props: { title: ReactNode; rows: { label: string; value: string; color?: string }[] }) {
  return (
    <div className="min-w-[160px] rounded-lg border border-line bg-surface px-3 py-2 text-xs shadow-lg">
      <div className="mb-1 font-semibold text-ink">{props.title}</div>
      {props.rows.map((r) => (
        <div key={r.label} className="flex items-center justify-between gap-4">
          <span className="flex items-center gap-1.5 text-ink-2">
            {r.color && <span className="inline-block h-2 w-2 rounded-full" style={{ background: r.color }} />}
            {r.label}
          </span>
          <span className="font-medium tabular-nums text-ink">{r.value}</span>
        </div>
      ))}
    </div>
  )
}

export function Sparkline({ points, alert }: { points: WeekPoint[]; alert?: boolean }) {
  const data = points.map((p) => ({ w: p.week, v: p.price }))
  return (
    <div className="h-8 w-28">
      <ResponsiveContainer>
        <LineChart data={data} margin={{ top: 3, bottom: 3, left: 2, right: 2 }}>
          <YAxis hide domain={['dataMin', 'dataMax']} />
          <Line type="monotone" dataKey="v" stroke={alert ? 'var(--critical)' : 'var(--series-1)'} strokeWidth={1.75} dot={false} connectNulls isAnimationActive={false} />
        </LineChart>
      </ResponsiveContainer>
    </div>
  )
}

export function Legend({ items }: { items: { label: string; color: string; dashed?: boolean }[] }) {
  return (
    <div className="flex flex-wrap gap-x-4 gap-y-1 text-xs text-ink-2">
      {items.map((i) => (
        <span key={i.label} className="inline-flex items-center gap-1.5">
          <svg width="16" height="8" aria-hidden>
            <line x1="0" y1="4" x2="16" y2="4" stroke={i.color} strokeWidth="2" strokeDasharray={i.dashed ? '3 3' : undefined} />
          </svg>
          {i.label}
        </span>
      ))}
    </div>
  )
}
