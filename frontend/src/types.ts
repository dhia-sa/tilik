export type Role = 'viewer' | 'data_admin' | 'admin'
export interface User { id: number; email: string; name: string; role: Role }
export interface Opt { id: number; name: string }
export interface Meta {
  reference_date: string
  default_from: string
  default_to: string
  data_from: string | null
  data_to: string | null
  ai_provider: string
  categories: Opt[]
  vendors: Opt[]
  commodities: (Opt & { base_unit: string })[]
}
export interface Filter { from: string; to: string; prev_from: string; prev_to: string; category: number | null; vendor: number | null }
export interface Kpi { total_spend: number; po_count: number; active_vendors: number; on_time_pct: number | null; delivered_po: number }
export interface MonthPoint { month: string; spend: number; po_count: number }
export interface AlertRow { id: number; kind: 'price' | 'vendor'; ref_id: number; ref_name: string; week: string; period_start: string; message: string; value: number }
export interface CategorySpend { id: number; name: string; spend: number; prev_spend: number; change_pct: number | null; share_pct: number }
export interface ItemSpend { item_id: number; item: string; category: string; spend: number; base_qty: number; base_unit: string; avg_price: number; po_count: number }
export interface SpendRow { id: number | null; name: string; spend: number; po_count: number; share_pct: number }
export interface ItemEffect {
  item_id: number; name: string; spend0: number; spend1: number; qty0: number; qty1: number; price0: number | null; price1: number | null
  volume_effect: number; price_effect: number; new_item_effect: number; lost_item_effect: number; total_change: number
}
export interface Decomposition {
  spend0: number; spend1: number; volume_effect: number; price_effect: number; new_item_effect: number; lost_item_effect: number; total_change: number
  items: ItemEffect[]
}
export interface WeekPoint { week: string; week_start: string; price: number | null }
export interface CommodityRow {
  id: number; name: string; base_unit: string; this_week: string; price_this_week: number | null; price_last_week: number | null
  change_pct: number | null; qty_this_week: number; sparkline: WeekPoint[]; alert: boolean; alert_message: string | null
}
export interface WeekDetail { week: string; week_start: string; price: number | null; qty: number; change_pct: number | null; mean12: number | null; upper_band: number | null; alert: boolean }
export interface VendorPrice { vendor_id: number; vendor: string; avg_price_4w: number | null; avg_price_52w: number; qty_52w: number; diff_vs_market_pct: number }
export interface VendorScore {
  vendor_id: number; vendor: string; spend: number; po_count: number; delivered: number; on_time: number; late: number
  on_time_pct: number | null; avg_quality: number | null; reject_pct: number | null; quality_score: number | null
  price_diff_pct: number | null; price_score: number | null; total: number | null; label: string
}
export interface Weights { on_time: number; quality: number; price: number }
export interface ToolTable { title: string; columns: string[]; rows: (string | number | null)[][]; note: string | null }
export interface AskResponse {
  answer: string
  verified: boolean
  fallback?: boolean
  fallback_reason?: string | null
  unanswerable?: boolean
  tool: { name: string; params: Record<string, unknown>; raw_params?: Record<string, unknown>; reason?: string } | null
  table: ToolTable | null
  attempts: { tool: string; params: unknown; valid: boolean; error?: string }[]
  provider: string
  duration_ms: number
  suggestions?: string[] | null
}
