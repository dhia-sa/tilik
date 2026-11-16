import { useEffect, useRef, useState, type FormEvent } from 'react'
import { api, postJson } from '../api'
import { Button, Card, ErrorBox, PageHeader, Spinner } from '../components/ui'
import { useApp } from '../context'
import { num } from '../format'
import type { AskResponse } from '../types'

interface HistoryItem {
  id: number
  question: string
  response: AskResponse | null
  created_at: string
}

const EXAMPLES = [
  'Kategori apa yang paling besar kuartal ini?',
  'Harga gula 8 minggu terakhir gimana?',
  'Vendor gula mana yang paling sering telat bulan ini?',
  'Tepung naik berapa persen dibanding bulan lalu?',
  '10 item kemasan dengan spend terbesar tahun ini?',
  'Ada alert harga apa saja minggu ini?',
  'Harga emas hari ini berapa?',
]

function cell(v: string | number | null, col: string): string {
  if (v === null || v === undefined) return '–'
  if (typeof v === 'number') {
    if (/peringkat|minggu$|jumlah po|po telat|po terkirim/i.test(col)) return num(v)
    if (/%/.test(col)) return num(v, 1)
    if (/skor/i.test(col)) return num(v, 1)
    return num(v)
  }
  return v
}

function Answer({ q, r }: { q: string; r: AskResponse }) {
  const [showTool, setShowTool] = useState(false)
  return (
    <div className="flex flex-col gap-2">
      <div className="ml-auto max-w-[80%] rounded-2xl rounded-br-sm bg-accent px-3.5 py-2 text-sm text-white">{q}</div>
      <div className="max-w-[95%] rounded-2xl rounded-bl-sm border border-line bg-surface px-4 py-3">
        <p className="text-sm leading-relaxed text-ink">{r.answer}</p>
        <div className="mt-2 flex flex-wrap items-center gap-2 text-[11px] text-muted">
          {r.unanswerable ? (
            <span>○ Di luar cakupan data</span>
          ) : r.table ? (
            <span className="text-good">● Angka diverifikasi terhadap tabel hasil</span>
          ) : null}
          {r.fallback && <span title={r.fallback_reason ?? ''}>· ringkasan otomatis ({r.fallback_reason})</span>}
          <span>
            · {r.provider} · {(r.duration_ms / 1000).toLocaleString('id-ID', { maximumFractionDigits: 1 })} dtk
          </span>
          {r.tool && (
            <button onClick={() => setShowTool((s) => !s)} className="rounded border border-line px-1.5 py-0.5 text-ink-2 hover:bg-surface-2">
              {showTool ? '▾' : '▸'} Tool: {r.tool.name}
            </button>
          )}
        </div>
        {showTool && r.tool && (
          <div className="mt-2 rounded-lg bg-surface-2 p-2 font-mono text-[11px] text-ink-2">
            <div>tool: {r.tool.name}</div>
            <div className="whitespace-pre-wrap">params: {JSON.stringify(r.tool.params, null, 2)}</div>
            {r.tool.reason && <div className="mt-1 font-sans">alasan AI: {r.tool.reason}</div>}
            {r.attempts.length > 1 && (
              <div className="mt-1 font-sans">
                Percobaan: {r.attempts.map((a, i) => <div key={i}>#{i + 1} {a.tool} {a.valid ? '✓' : `✗ ${a.error}`}</div>)}
              </div>
            )}
          </div>
        )}
        {r.table && r.table.rows.length > 0 && (
          <div className="mt-3">
            <div className="mb-1 text-xs font-medium text-ink-2">{r.table.title}</div>
            <div className="max-h-72 overflow-auto rounded-lg border border-line">
              <table className="w-full text-xs">
                <thead className="sticky top-0 bg-surface-2">
                  <tr>
                    {r.table.columns.map((c) => (
                      <th key={c} className="whitespace-nowrap px-2 py-1.5 text-left font-medium text-ink-2">
                        {c}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {r.table.rows.map((row, i) => (
                    <tr key={i} className="border-t border-line">
                      {row.map((v, j) => (
                        <td key={j} className={`whitespace-nowrap px-2 py-1 ${typeof v === 'number' ? 'text-right tabular-nums' : ''}`}>
                          {cell(v, r.table?.columns[j] ?? '')}
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {r.table.note && <div className="mt-1 text-[11px] text-muted">{r.table.note}</div>}
          </div>
        )}
        {r.suggestions && r.suggestions.length > 0 && (
          <div className="mt-2 text-xs text-ink-2">Coba tanyakan: {r.suggestions.slice(0, 3).join(' · ')}</div>
        )}
      </div>
    </div>
  )
}

export default function AskPage() {
  const { meta } = useApp()
  const [question, setQuestion] = useState('')
  const [items, setItems] = useState<{ q: string; r: AskResponse | null; err?: string }[]>([])
  const [history, setHistory] = useState<HistoryItem[]>([])
  const [busy, setBusy] = useState(false)
  const bottom = useRef<HTMLDivElement>(null)

  const loadHistory = () =>
    api<{ history: HistoryItem[] }>('/ask/history')
      .then((h) => setHistory(h.history))
      .catch(() => setHistory([]))
  useEffect(() => {
    loadHistory()
  }, [])
  useEffect(() => {
    bottom.current?.scrollIntoView({ behavior: 'smooth' })
  }, [items])

  async function ask(q: string, e?: FormEvent) {
    e?.preventDefault()
    const text = q.trim()
    if (!text || busy) return
    setQuestion('')
    setBusy(true)
    setItems((xs) => [...xs, { q: text, r: null }])
    try {
      const r = await postJson<AskResponse>('/ask', { question: text })
      setItems((xs) => xs.map((x, i) => (i === xs.length - 1 ? { q: text, r } : x)))
      loadHistory()
    } catch (err) {
      setItems((xs) => xs.map((x, i) => (i === xs.length - 1 ? { q: text, r: null, err: (err as Error).message } : x)))
    } finally {
      setBusy(false)
    }
  }

  return (
    <>
      <PageHeader
        title="Tanya Data"
        subtitle={`AI memilih tool terdefinisi (bukan SQL bebas); angka di jawaban hanya dari tabel hasil query. Provider: ${meta.ai_provider} · tanggal acuan ${meta.reference_date}`}
      />
      <div className="grid grid-cols-1 gap-4 xl:grid-cols-[1fr_300px]">
        <Card className="flex min-h-[560px] flex-col">
          <div className="flex-1 space-y-5 overflow-auto pb-4">
            {items.length === 0 && (
              <div className="py-10 text-center">
                <div className="text-sm font-medium text-ink">Tanyakan apa saja tentang spend, harga komoditas, vendor, atau alert.</div>
                <div className="mx-auto mt-4 flex max-w-2xl flex-wrap justify-center gap-2">
                  {EXAMPLES.map((ex) => (
                    <button key={ex} onClick={() => ask(ex)} className="rounded-full border border-line px-3 py-1.5 text-xs text-ink-2 hover:bg-surface-2 hover:text-ink">
                      {ex}
                    </button>
                  ))}
                </div>
              </div>
            )}
            {items.map((it, i) => (
              <div key={i}>
                {it.r ? (
                  <Answer q={it.q} r={it.r} />
                ) : (
                  <div className="flex flex-col gap-2">
                    <div className="ml-auto max-w-[80%] rounded-2xl rounded-br-sm bg-accent px-3.5 py-2 text-sm text-white">{it.q}</div>
                    {it.err ? (
                      <ErrorBox message={it.err} />
                    ) : (
                      <div className="flex items-center gap-2 text-sm text-muted">
                        <Spinner /> Memilih tool, menjalankan query, dan merangkum…
                      </div>
                    )}
                  </div>
                )}
              </div>
            ))}
            <div ref={bottom} />
          </div>
          <form onSubmit={(e) => ask(question, e)} className="flex gap-2 border-t border-line pt-3">
            <input
              value={question}
              onChange={(e) => setQuestion(e.target.value)}
              maxLength={500}
              placeholder="mis. Vendor kemasan mana yang paling banyak reject?"
              className="flex-1 rounded-lg border border-line bg-surface px-3 py-2 text-sm text-ink outline-none focus:border-accent"
            />
            <Button type="submit" variant="primary" disabled={busy || !question.trim()}>
              {busy ? 'Memproses…' : 'Tanya'}
            </Button>
          </form>
        </Card>
        <div className="flex flex-col gap-4">
          <Card title="Contoh pertanyaan">
            <div className="flex flex-col gap-1">
              {EXAMPLES.map((ex) => (
                <button key={ex} onClick={() => ask(ex)} disabled={busy} className="rounded-lg px-2 py-1.5 text-left text-xs text-ink-2 hover:bg-surface-2 hover:text-ink">
                  {ex}
                </button>
              ))}
            </div>
          </Card>
          <Card title="Riwayat pertanyaan" subtitle="Klik untuk menampilkan ulang">
            {history.length === 0 ? (
              <p className="text-xs text-muted">Belum ada riwayat.</p>
            ) : (
              <div className="flex max-h-96 flex-col gap-1 overflow-auto">
                {history.map((h) => (
                  <button
                    key={h.id}
                    onClick={() => h.response && setItems((xs) => [...xs, { q: h.question, r: h.response }])}
                    className="rounded-lg px-2 py-1.5 text-left text-xs hover:bg-surface-2"
                  >
                    <div className="text-ink">{h.question}</div>
                    <div className="text-[11px] text-muted">
                      {h.created_at} · {h.response?.tool?.name ?? 'tidak terjawab'}
                    </div>
                  </button>
                ))}
              </div>
            )}
          </Card>
        </div>
      </div>
    </>
  )
}
