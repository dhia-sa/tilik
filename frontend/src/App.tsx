import { useCallback, useEffect, useState } from 'react'
import { Navigate, Route, Routes } from 'react-router-dom'
import { api, setUnauthorizedHandler } from './api'
import { Layout } from './components/Layout'
import { ErrorBox, Loading } from './components/ui'
import { AppProvider, useApp } from './context'
import type { User } from './types'
import LoginPage from './pages/Login'
import OverviewPage from './pages/Overview'
import SpendPage from './pages/Spend'
import CommoditiesPage from './pages/Commodities'
import CommodityDetailPage from './pages/CommodityDetail'
import VendorsPage from './pages/Vendors'
import VendorDetailPage from './pages/VendorDetail'
import AskPage from './pages/Ask'
import ImportPage from './pages/Import'
import SettingsPage from './pages/Settings'

const THEME_KEY = 'tilik-theme'

/** Default terang (desain utama); pilihan pengguna diingat di browser. */
function initialTheme(): 'light' | 'dark' {
  try {
    const saved = localStorage.getItem(THEME_KEY)
    if (saved === 'light' || saved === 'dark') return saved
  } catch {
    /* storage tidak tersedia */
  }
  return 'light'
}

function AdminOnly({ children }: { children: React.ReactNode }) {
  const { user } = useApp()
  if (user.role === 'viewer') return <ErrorBox message="Halaman ini hanya untuk Data Admin / Admin." />
  return <>{children}</>
}

export default function App() {
  const [user, setUser] = useState<User | null | undefined>(undefined)
  const [theme, setTheme] = useState<'light' | 'dark'>(initialTheme)

  useEffect(() => {
    document.documentElement.dataset.theme = theme
    try {
      localStorage.setItem(THEME_KEY, theme)
    } catch {
      /* storage tidak tersedia */
    }
  }, [theme])

  useEffect(() => {
    setUnauthorizedHandler(() => setUser(null))
    api<User>('/auth/me')
      .then(setUser)
      .catch(() => setUser(null))
  }, [])

  const logout = useCallback(() => {
    api('/auth/logout', { method: 'POST' }).finally(() => setUser(null))
  }, [])
  const toggleTheme = useCallback(() => setTheme((t) => (t === 'dark' ? 'light' : 'dark')), [])

  if (user === undefined) return <Loading label="Memeriksa sesi…" height="h-screen" />
  if (user === null) return <LoginPage onLogin={setUser} theme={theme} toggleTheme={toggleTheme} />

  return (
    <AppProvider user={user} logout={logout} theme={theme} toggleTheme={toggleTheme}>
      {(state, err) =>
        state === 'loading' ? (
          <Loading label="Menyiapkan dashboard…" height="h-screen" />
        ) : state === 'error' ? (
          <div className="p-8">
            <ErrorBox message={err ?? 'Gagal memuat data'} onRetry={() => window.location.reload()} />
          </div>
        ) : (
          <Layout>
            <Routes>
              <Route path="/" element={<OverviewPage />} />
              <Route path="/spend" element={<SpendPage />} />
              <Route path="/spend/:id" element={<SpendPage />} />
              <Route path="/komoditas" element={<CommoditiesPage />} />
              <Route path="/komoditas/:id" element={<CommodityDetailPage />} />
              <Route path="/vendor" element={<VendorsPage />} />
              <Route path="/vendor/:id" element={<VendorDetailPage />} />
              <Route path="/tanya" element={<AskPage />} />
              <Route path="/import" element={<AdminOnly><ImportPage /></AdminOnly>} />
              <Route path="/pengaturan" element={<AdminOnly><SettingsPage /></AdminOnly>} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </Layout>
        )
      }
    </AppProvider>
  )
}
