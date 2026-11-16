// Format angka & tanggal gaya Indonesia.

const idNum = new Intl.NumberFormat('id-ID', { maximumFractionDigits: 0 })

export function rp(v: number | null | undefined): string {
  if (v === null || v === undefined || Number.isNaN(v)) return '–'
  return `Rp ${idNum.format(Math.round(v))}`
}

/** Rupiah ringkas untuk sumbu/kartu: Rp 1,23 M / Rp 450 jt */
export function rpShort(v: number | null | undefined): string {
  if (v === null || v === undefined || Number.isNaN(v)) return '–'
  const a = Math.abs(v)
  const f = (x: number, d: number) => x.toLocaleString('id-ID', { maximumFractionDigits: d, minimumFractionDigits: 0 })
  if (a >= 1e12) return `Rp ${f(v / 1e12, 2)} T`
  if (a >= 1e9) return `Rp ${f(v / 1e9, 2)} M`
  if (a >= 1e6) return `Rp ${f(v / 1e6, 1)} jt`
  if (a >= 1e3) return `Rp ${f(v / 1e3, 0)} rb`
  return `Rp ${f(v, 0)}`
}

export function num(v: number | null | undefined, d = 0): string {
  if (v === null || v === undefined || Number.isNaN(v)) return '–'
  return v.toLocaleString('id-ID', { maximumFractionDigits: d, minimumFractionDigits: d })
}

export function pct(v: number | null | undefined, d = 1, signed = false): string {
  if (v === null || v === undefined || Number.isNaN(v)) return '–'
  const s = v.toLocaleString('id-ID', { maximumFractionDigits: d, minimumFractionDigits: d })
  return `${signed && v > 0 ? '+' : ''}${s}%`
}

const BULAN = ['Jan', 'Feb', 'Mar', 'Apr', 'Mei', 'Jun', 'Jul', 'Agu', 'Sep', 'Okt', 'Nov', 'Des']

export function monthLabel(ym: string): string {
  const [y, m] = ym.split('-')
  const i = Number(m) - 1
  return BULAN[i] ? `${BULAN[i]} ${y.slice(2)}` : ym
}

export function dateLabel(d: string | null | undefined): string {
  if (!d) return '–'
  const [y, m, day] = d.split('-')
  const i = Number(m) - 1
  return BULAN[i] ? `${Number(day)} ${BULAN[i]} ${y}` : d
}

export function change(cur: number | null | undefined, prev: number | null | undefined): number | null {
  if (cur === null || cur === undefined || !prev) return null
  return (cur / prev - 1) * 100
}
