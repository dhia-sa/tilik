import { useState, type FormEvent } from 'react'
import { postJson } from '../api'
import { Button, ErrorBox, Input } from '../components/ui'
import type { User } from '../types'

const DEMO = [
  { label: 'Viewer', email: 'viewer@spendview.local', pw: 'viewer123' },
  { label: 'Data Admin', email: 'dataadmin@spendview.local', pw: 'dataadmin123' },
  { label: 'Admin IT', email: 'admin@spendview.local', pw: 'admin123' },
]

export default function LoginPage(props: { onLogin: (u: User) => void; theme: string; toggleTheme: () => void }) {
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  async function submit(e?: FormEvent, override?: { email: string; pw: string }) {
    e?.preventDefault()
    setBusy(true)
    setError(null)
    try {
      const u = await postJson<User>('/auth/login', { email: override?.email ?? email, password: override?.pw ?? password })
      props.onLogin(u)
    } catch (err) {
      setError((err as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center px-4">
      <div className="w-full max-w-sm">
        <div className="mb-6 flex items-center gap-3">
          <svg width="36" height="36" viewBox="0 0 32 32" aria-hidden>
            <rect width="32" height="32" rx="8" fill="var(--accent)" />
            <path d="M8 22l6-7 4 4 6-9" stroke="white" strokeWidth="3" fill="none" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          <div>
            <h1 className="text-lg font-semibold text-ink">SpendView</h1>
            <p className="text-xs text-muted">Spend, harga komoditas & performa vendor</p>
          </div>
        </div>
        <form onSubmit={submit} className="flex flex-col gap-3 rounded-xl border border-line bg-surface p-5">
          <h2 className="text-sm font-semibold text-ink">Masuk</h2>
          <Input label="Email" type="email" value={email} onChange={setEmail} placeholder="nama@perusahaan.local" />
          <Input label="Password" type="password" value={password} onChange={setPassword} />
          {error && <ErrorBox message={error} />}
          <Button type="submit" variant="primary" disabled={busy || !email || !password} className="justify-center">
            {busy ? 'Memproses…' : 'Masuk'}
          </Button>
        </form>
        <div className="mt-4 rounded-xl border border-dashed border-line p-4 text-xs text-ink-2">
          <div className="mb-2 font-medium">Akun demo (data dummy)</div>
          <div className="flex flex-wrap gap-2">
            {DEMO.map((d) => (
              <Button key={d.email} onClick={() => submit(undefined, { email: d.email, pw: d.pw })} disabled={busy}>
                Masuk sebagai {d.label}
              </Button>
            ))}
          </div>
        </div>
        <button onClick={props.toggleTheme} className="mt-4 text-xs text-muted hover:text-ink">
          {props.theme === 'dark' ? '☀ Mode terang' : '☾ Mode gelap'}
        </button>
      </div>
    </div>
  )
}
