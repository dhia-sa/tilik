import { useEffect, useState, type ReactNode } from 'react'
import { NavLink, useLocation } from 'react-router-dom'
import { LayoutDashboard, LogOut, Menu, Moon, X, ScanEye, Settings, Sparkles, Store, Sun, Upload, Wallet, Wheat, type LucideIcon } from 'lucide-react'
import { useApp } from '../context'
import type { Role } from '../types'

const NAV: { group: string; items: { to: string; label: string; icon: LucideIcon; roles?: Role[] }[] }[] = [
  {
    group: 'Menu',
    items: [
      { to: '/', label: 'Ringkasan', icon: LayoutDashboard },
      { to: '/spend', label: 'Spend', icon: Wallet },
      { to: '/komoditas', label: 'Komoditas', icon: Wheat },
      { to: '/vendor', label: 'Vendor', icon: Store },
      { to: '/tanya', label: 'Tanya Data', icon: Sparkles },
    ],
  },
  {
    group: 'Data',
    items: [
      { to: '/import', label: 'Import', icon: Upload, roles: ['data_admin', 'admin'] },
      { to: '/pengaturan', label: 'Pengaturan', icon: Settings, roles: ['data_admin', 'admin'] },
    ],
  },
]

const ROLE_LABEL: Record<Role, string> = { viewer: 'Viewer', data_admin: 'Data Admin', admin: 'Admin IT' }

function initials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0]?.toUpperCase())
    .join('')
}

export function Logo({ size = 36 }: { size?: number }) {
  return (
    <span className="grid shrink-0 place-items-center rounded-[10px] bg-accent text-white" style={{ width: size, height: size }}>
      <ScanEye size={Math.round(size * 0.56)} strokeWidth={2} aria-hidden />
    </span>
  )
}

export function Layout({ children }: { children: ReactNode }) {
  const { user, logout, theme, toggleTheme, meta } = useApp()
  const [open, setOpen] = useState(false)
  const { pathname } = useLocation()
  useEffect(() => setOpen(false), [pathname])
  return (
    <div className="flex min-h-screen">
      <div className="fixed inset-x-0 top-0 z-30 flex h-14 items-center gap-3 border-b border-line bg-surface px-4 lg:hidden">
        <button onClick={() => setOpen(true)} aria-label="Buka menu" className="grid h-9 w-9 place-items-center rounded-lg text-ink-2 hover:bg-surface-2 hover:text-ink">
          <Menu size={20} />
        </button>
        <Logo size={28} />
        <span className="font-display text-base font-bold text-ink">Tilik</span>
      </div>
      {open && <div className="fixed inset-0 z-40 bg-[#0f172a]/40 lg:hidden" onClick={() => setOpen(false)} aria-hidden />}
      <aside
        className={`fixed inset-y-0 left-0 z-50 flex h-screen w-64 shrink-0 flex-col border-r border-line bg-surface transition-transform duration-300 ease-[cubic-bezier(0.4,0,0.2,1)] lg:sticky lg:top-0 lg:translate-x-0 ${
          open ? 'translate-x-0' : '-translate-x-full'
        }`}
      >
        <button onClick={() => setOpen(false)} aria-label="Tutup menu" className="absolute right-3 top-6 grid h-8 w-8 place-items-center rounded-lg text-ink-2 hover:bg-surface-2 lg:hidden">
          <X size={18} />
        </button>
        <div className="flex items-center gap-2.5 px-5 py-6">
          <Logo />
          <div>
            <div className="font-display text-lg font-bold leading-none text-ink">Tilik</div>
            <div className="mt-1 text-xs text-muted">Analitik pengadaan</div>
          </div>
        </div>
        <nav className="flex flex-1 flex-col overflow-auto px-3 pb-3">
          {NAV.map((g) => {
            const items = g.items.filter((n) => !n.roles || n.roles.includes(user.role))
            if (items.length === 0) return null
            return (
              <div key={g.group} className="flex flex-col gap-0.5">
                <div className="px-2.5 pb-1.5 pt-4 text-[11px] font-medium uppercase tracking-wider text-muted">{g.group}</div>
                {items.map((n) => (
                  <NavLink
                    key={n.to}
                    to={n.to}
                    end={n.to === '/'}
                    className={({ isActive }) =>
                      `flex items-center gap-2 border-r-4 p-2.5 text-sm font-medium ${
                        isActive ? 'rounded-l-[10px] border-accent bg-surface-2 text-accent' : 'rounded-[10px] border-transparent text-ink-2 hover:bg-bg hover:text-ink'
                      }`
                    }
                  >
                    <n.icon size={20} strokeWidth={1.75} aria-hidden />
                    {n.label}
                  </NavLink>
                ))}
              </div>
            )
          })}
        </nav>
        <div className="m-3 rounded-2xl border border-line p-3">
          <div className="flex items-center gap-2.5">
            <span className="grid h-10 w-10 shrink-0 place-items-center rounded-full bg-crit-bg font-display text-sm font-bold text-crit">{initials(user.name)}</span>
            <div className="min-w-0 flex-1">
              <div className="truncate text-sm font-medium text-ink">{user.name}</div>
              <div className="truncate text-xs text-ink-2">
                {ROLE_LABEL[user.role]} · AI {meta.ai_provider}
              </div>
            </div>
          </div>
          <div className="mt-3 flex gap-1 border-t border-line pt-2">
            <button onClick={toggleTheme} className="flex flex-1 items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs font-medium text-ink-2 hover:bg-surface-2 hover:text-ink" title="Ganti tema">
              {theme === 'dark' ? <Sun size={16} aria-hidden /> : <Moon size={16} aria-hidden />}
              {theme === 'dark' ? 'Mode terang' : 'Mode gelap'}
            </button>
            <button onClick={logout} className="flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-xs font-medium text-ink-2 hover:bg-crit-bg hover:text-crit" title="Keluar">
              <LogOut size={16} aria-hidden />
              Keluar
            </button>
          </div>
        </div>
      </aside>
      <main className="min-w-0 flex-1 px-4 pb-12 pt-20 sm:px-6 lg:px-10 lg:pt-8">{children}</main>
    </div>
  )
}
