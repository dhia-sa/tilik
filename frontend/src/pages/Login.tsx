import { useState, type FormEvent } from 'react'
import { ArrowRight, Moon, ShieldCheck, Sun, UserRound, Wrench } from 'lucide-react'
import { postJson } from '../api'
import { Logo } from '../components/Layout'
import { Button, ErrorBox, Input } from '../components/ui'
import type { User } from '../types'

const DEMO = [
  { label: 'Viewer', desc: 'Semua dashboard dan Tanya Data', email: 'viewer@tilik.local', pw: 'viewer123', icon: UserRound },
  { label: 'Data Admin', desc: '+ Import, rollback, pengaturan', email: 'dataadmin@tilik.local', pw: 'dataadmin123', icon: ShieldCheck },
  { label: 'Admin IT', desc: '+ Kelola user dan role', email: 'admin@tilik.local', pw: 'admin123', icon: Wrench },
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
    <div className="flex min-h-screen items-center justify-center px-4 py-10">
      <div className="w-full max-w-md">
        <div className="mb-8 flex items-center gap-3">
          <Logo size={44} />
          <div>
            <h1 className="text-2xl leading-none text-ink">Tilik</h1>
            <p className="mt-1.5 text-sm text-ink-2">Spend, harga komoditas, dan performa vendor</p>
          </div>
        </div>
        <form onSubmit={submit} className="flex flex-col gap-4 rounded-2xl border border-line bg-surface p-6">
          <h2 className="text-base text-ink">Masuk</h2>
          <Input label="Email" type="email" value={email} onChange={setEmail} placeholder="nama@perusahaan.local" />
          <Input label="Password" type="password" value={password} onChange={setPassword} />
          {error && <ErrorBox message={error} />}
          <Button type="submit" variant="primary" disabled={busy || !email || !password} className="justify-center">
            {busy ? 'Memproses…' : 'Masuk'}
          </Button>
        </form>
        <div className="mt-4 rounded-2xl border border-line bg-surface p-6">
          <div className="mb-3 flex items-center justify-between">
            <span className="font-display text-sm font-bold text-ink">Akun demo</span>
            <span className="rounded-full bg-surface-2 px-2 py-0.5 text-xs font-medium text-ink-2">data dummy</span>
          </div>
          <div className="flex flex-col gap-2">
            {DEMO.map((d) => (
              <button
                key={d.email}
                onClick={() => submit(undefined, { email: d.email, pw: d.pw })}
                disabled={busy}
                className="group flex items-center gap-3 rounded-xl border border-line px-3 py-2.5 text-left hover:border-line-strong hover:bg-bg disabled:opacity-50"
              >
                <span className="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-crit-bg text-crit">
                  <d.icon size={18} strokeWidth={1.75} aria-hidden />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-sm font-medium text-ink">Masuk sebagai {d.label}</span>
                  <span className="block text-xs text-ink-2">{d.desc}</span>
                </span>
                <ArrowRight size={16} className="text-muted group-hover:text-accent" aria-hidden />
              </button>
            ))}
          </div>
        </div>
        <button onClick={props.toggleTheme} className="mt-4 inline-flex items-center gap-1.5 text-xs font-medium text-ink-2 hover:text-ink">
          {props.theme === 'dark' ? <Sun size={14} aria-hidden /> : <Moon size={14} aria-hidden />}
          {props.theme === 'dark' ? 'Mode terang' : 'Mode gelap'}
        </button>
      </div>
    </div>
  )
}
