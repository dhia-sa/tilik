// Mode demo (build dengan VITE_DEMO=1): tanpa backend. Setiap GET dibaca dari snapshot JSON
// di folder demo/ yang dibuat oleh scripts/snapshot_demo.py dari server lokal. Semua aksi yang
// mengubah data ditolak dengan pesan yang jelas.

export const DEMO = import.meta.env.VITE_DEMO === '1'

export const DEMO_USER = { id: 0, email: 'demo@tilik.local', name: 'Demo Tilik', role: 'data_admin' as const }

export const DEMO_READONLY_MSG = 'Ini versi demo hanya-baca. Fitur ini butuh backend: jalankan Tilik secara lokal (lihat README di GitHub).'

/** Nama file snapshot untuk path API (query diurutkan, nilai kosong dibuang). Harus sama dengan scripts/snapshot_demo.py. */
export function demoKey(path: string): string {
  const [p, q = ''] = path.split('?')
  const entries = [...new URLSearchParams(q).entries()].filter(([, v]) => v !== '').sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
  const full = p + (entries.length ? '?' + entries.map(([k, v]) => `${k}=${v}`).join('&') : '')
  return full.replace(/[^A-Za-z0-9]+/g, '_').replace(/^_+|_+$/g, '').toLowerCase()
}

export function demoAskKey(question: string): string {
  return demoKey(`/ask?q=${question.trim().toLowerCase()}`)
}

export async function demoGet<T>(path: string): Promise<T> {
  const res = await fetch(`${import.meta.env.BASE_URL}demo/${demoKey(path)}.json`)
  if (!res.ok) {
    throw new Error('Data untuk tampilan ini tidak ada di versi demo. Pakai preset periode tanpa filter kategori/vendor, atau jalankan Tilik secara lokal.')
  }
  return (await res.json()) as T
}

export async function demoAsk<T>(question: string): Promise<T> {
  const res = await fetch(`${import.meta.env.BASE_URL}demo/${demoAskKey(question)}.json`)
  if (!res.ok) throw new Error('Di versi demo, Tanya Data hanya menjawab contoh pertanyaan di sebelah kanan. Jalankan Tilik secara lokal untuk bertanya bebas.')
  return (await res.json()) as T
}
