import type { ReactNode } from 'react'
import { NavLink } from 'react-router-dom'
import { useApp } from '../context'
import type { Role } from '../types'

const NAV: { to: string; label: string; icon: string; roles?: Role[] }[] = [
  { to: '/', label: 'Overview', icon: 'M3 12l9-8 9 8M5 10v10h14V10' },
  { to: '/spend', label: 'Spend', icon: 'M4 20V10M10 20V4M16 20v-7M22 20H2' },
  { to: '/komoditas', label: 'Komoditas', icon: 'M3 17l6-6 4 4 8-8M15 7h6v6' },
  { to: '/vendor', label: 'Vendor', icon: 'M4 21V8l8-5 8 5v13M9 21v-6h6v6' },
  { to: '/tanya', label: 'Tanya Data', icon: 'M21 12a8 8 0 01-11.6 7.1L3 21l1.9-6.4A8 8 0 1121 12z' },
  { to: '/import', label: 'Import', icon: 'M12 3v12m0 0l-4-4m4 4l4-4M4 17v3h16v-3', roles: ['data_admin', 'admin'] },
  { to: '/pengaturan', label: 'Pengaturan', icon: 'M12 15a3 3 0 100-6 3 3 0 000 6zM19.4 15a1.7 1.7 0 00.3 1.8l.1.1a2 2 0 11-2.8 2.8l-.1-.1a1.7 1.7 0 00-1.8-.3 1.7 1.7 0 00-1 1.5V21a2 2 0 11-4 0v-.1a1.7 1.7 0 00-1.1-1.5 1.7 1.7 0 00-1.8.3l-.1.1a2 2 0 11-2.8-2.8l.1-.1a1.7 1.7 0 00.3-1.8 1.7 1.7 0 00-1.5-1H3a2 2 0 110-4h.1a1.7 1.7 0 001.5-1.1 1.7 1.7 0 00-.3-1.8l-.1-.1a2 2 0 112.8-2.8l.1.1a1.7 1.7 0 001.8.3H9a1.7 1.7 0 001-1.5V3a2 2 0 114 0v.1a1.7 1.7 0 001 1.5 1.7 1.7 0 001.8-.3l.1-.1a2 2 0 112.8 2.8l-.1.1a1.7 1.7 0 00-.3 1.8V9a1.7 1.7 0 001.5 1H21a2 2 0 110 4h-.1a1.7 1.7 0 00-1.5 1z', roles: ['data_admin', 'admin'] },
]

const ROLE_LABEL: Record<Role, string> = { viewer: 'Viewer', data_admin: 'Data Admin', admin: 'Admin IT' }

export function Layout({ children }: { children: ReactNode }) {
  const { user, logout, theme, toggleTheme, meta } = useApp()
  return (
    <div className="flex min-h-screen">
      <aside className="sticky top-0 flex h-screen w-56 shrink-0 flex-col border-r border-line bg-surface px-3 py-4">
        <div className="mb-6 flex items-center gap-2 px-2">
          <svg width="28" height="28" viewBox="0 0 32 32" aria-hidden>
            <rect width="32" height="32" rx="8" fill="var(--accent)" />
            <path d="M8 22l6-7 4 4 6-9" stroke="white" strokeWidth="3" fill="none" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          <div>
            <div className="text-sm font-semibold leading-tight text-ink">SpendView</div>
            <div className="text-[11px] text-muted">Procurement & Finance</div>
          </div>
        </div>
        <nav className="flex flex-1 flex-col gap-0.5">
          {NAV.filter((n) => !n.roles || n.roles.includes(user.role)).map((n) => (
            <NavLink
              key={n.to}
              to={n.to}
              end={n.to === '/'}
              className={({ isActive }) =>
                `flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm ${isActive ? 'bg-surface-2 font-medium text-ink' : 'text-ink-2 hover:bg-surface-2 hover:text-ink'}`
              }
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
                <path d={n.icon} />
              </svg>
              {n.label}
            </NavLink>
          ))}
        </nav>
        <div className="mt-4 border-t border-line pt-3">
          <div className="px-2 text-sm font-medium text-ink">{user.name}</div>
          <div className="px-2 text-xs text-muted">
            {ROLE_LABEL[user.role]} · AI: {meta.ai_provider}
          </div>
          <div className="mt-2 flex gap-1">
            <button onClick={toggleTheme} className="flex-1 rounded-lg px-2 py-1.5 text-left text-xs text-ink-2 hover:bg-surface-2" title="Ganti tema">
              {theme === 'dark' ? '☀ Mode terang' : '☾ Mode gelap'}
            </button>
            <button onClick={logout} className="rounded-lg px-2 py-1.5 text-xs text-ink-2 hover:bg-surface-2">
              Keluar
            </button>
          </div>
        </div>
      </aside>
      <main className="min-w-0 flex-1 px-6 py-5">{children}</main>
    </div>
  )
}
