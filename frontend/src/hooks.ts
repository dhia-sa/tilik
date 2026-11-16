import { useCallback, useEffect, useRef, useState } from 'react'
import { api } from './api'

export interface Async<T> {
  data: T | null
  error: string | null
  loading: boolean
  reload: () => void
}

/** Ambil data dari API setiap kali `path` berubah. `path = null` berarti jangan ambil. */
export function useApi<T>(path: string | null): Async<T> {
  const [data, setData] = useState<T | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState<boolean>(!!path)
  const [tick, setTick] = useState(0)
  const seq = useRef(0)
  useEffect(() => {
    if (!path) return
    const my = ++seq.current
    setLoading(true)
    setError(null)
    api<T>(path)
      .then((d) => {
        if (my === seq.current) setData(d)
      })
      .catch((e: Error) => {
        if (my === seq.current) setError(e.message)
      })
      .finally(() => {
        if (my === seq.current) setLoading(false)
      })
  }, [path, tick])
  const reload = useCallback(() => setTick((t) => t + 1), [])
  return { data, error, loading, reload }
}
