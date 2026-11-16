import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from 'react'
import { api, qs } from './api'
import type { Meta, User } from './types'

interface AppCtx {
  user: User
  meta: Meta
  reloadMeta: () => void
  filter: GlobalFilter
  setFilter: (f: GlobalFilter) => void
  filterQs: string
  logout: () => void
  theme: 'light' | 'dark'
  toggleTheme: () => void
}

export interface GlobalFilter {
  from: string
  to: string
  category: string
  vendor: string
}

const Ctx = createContext<AppCtx | null>(null)

export function useApp(): AppCtx {
  const c = useContext(Ctx)
  if (!c) throw new Error('useApp harus di dalam AppProvider')
  return c
}

export function AppProvider(props: {
  user: User
  logout: () => void
  theme: 'light' | 'dark'
  toggleTheme: () => void
  children: (state: 'loading' | 'error' | 'ready', err?: string) => ReactNode
}) {
  const [meta, setMeta] = useState<Meta | null>(null)
  const [err, setErr] = useState<string | null>(null)
  const [filter, setFilter] = useState<GlobalFilter | null>(null)
  const [tick, setTick] = useState(0)
  useEffect(() => {
    api<Meta>('/meta')
      .then((m) => {
        setMeta(m)
        setFilter((f) => f ?? { from: m.default_from, to: m.default_to, category: '', vendor: '' })
      })
      .catch((e: Error) => setErr(e.message))
  }, [tick])
  const value = useMemo<AppCtx | null>(() => {
    if (!meta || !filter) return null
    return {
      user: props.user,
      meta,
      reloadMeta: () => setTick((t) => t + 1),
      filter,
      setFilter,
      filterQs: qs({ from: filter.from, to: filter.to, category: filter.category, vendor: filter.vendor }),
      logout: props.logout,
      theme: props.theme,
      toggleTheme: props.toggleTheme,
    }
  }, [meta, filter, props.user, props.logout, props.theme, props.toggleTheme])
  if (err) return <>{props.children('error', err)}</>
  if (!value) return <>{props.children('loading')}</>
  return <Ctx.Provider value={value}>{props.children('ready')}</Ctx.Provider>
}
