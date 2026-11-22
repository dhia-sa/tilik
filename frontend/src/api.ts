// Pembungkus fetch: kirim cookie sesi, ubah error JSON backend jadi Error berpesan Indonesia.
import { DEMO, DEMO_READONLY_MSG, DEMO_USER, demoAsk, demoGet } from './demo'

export class ApiError extends Error {
  status: number
  constructor(message: string, status: number) {
    super(message)
    this.status = status
  }
}

let onUnauthorized: (() => void) | null = null
export function setUnauthorizedHandler(fn: () => void) {
  onUnauthorized = fn
}

export async function api<T>(path: string, init?: RequestInit): Promise<T> {
  if (DEMO) return demoApi<T>(path, init)
  let res: Response
  try {
    res = await fetch(`/api${path}`, { credentials: 'same-origin', ...init })
  } catch {
    throw new ApiError('Tidak dapat terhubung ke server. Pastikan backend berjalan.', 0)
  }
  if (res.status === 401 && !path.startsWith('/auth/')) onUnauthorized?.()
  const text = await res.text()
  let body: unknown = null
  try {
    body = text ? JSON.parse(text) : null
  } catch {
    body = null
  }
  if (!res.ok) {
    const msg = (body as { error?: string } | null)?.error ?? `Permintaan gagal (${res.status})`
    throw new ApiError(msg, res.status)
  }
  return body as T
}

async function demoApi<T>(path: string, init?: RequestInit): Promise<T> {
  const method = (init?.method ?? 'GET').toUpperCase()
  if (path === '/auth/me' || path === '/auth/login') return DEMO_USER as T
  if (path === '/auth/logout') return null as T
  if (method === 'GET') {
    try {
      return await demoGet<T>(path)
    } catch (e) {
      throw new ApiError((e as Error).message, 404)
    }
  }
  if (path === '/ask' && typeof init?.body === 'string') {
    try {
      return await demoAsk<T>((JSON.parse(init.body) as { question: string }).question)
    } catch (e) {
      throw new ApiError((e as Error).message, 404)
    }
  }
  throw new ApiError(DEMO_READONLY_MSG, 403)
}

export function postJson<T>(path: string, data: unknown, method = 'POST'): Promise<T> {
  return api<T>(path, { method, headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(data) })
}

export function qs(params: Record<string, string | number | null | undefined>): string {
  const p = new URLSearchParams()
  for (const [k, v] of Object.entries(params)) if (v !== null && v !== undefined && v !== '') p.set(k, String(v))
  const s = p.toString()
  return s ? `?${s}` : ''
}
