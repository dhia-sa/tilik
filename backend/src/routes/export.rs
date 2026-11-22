//! Export Excel (.xlsx) untuk halaman Spend, Komoditas, dan Vendor sesuai filter aktif.

use crate::auth::CurrentUser;
use crate::error::{AppError, AppResult};
use crate::filters::{reference_date, resolve, FilterQuery};
use crate::queries::{self, Scope};
use crate::settings::Settings;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::IntoResponse;
use rust_xlsxwriter::{Format, Workbook, Worksheet, XlsxError};

fn xe(e: XlsxError) -> AppError {
    AppError::internal(e)
}

struct Sheet<'a> {
    ws: &'a mut Worksheet,
    row: u32,
    money: Format,
    pct: Format,
    num: Format,
    bold: Format,
}

impl<'a> Sheet<'a> {
    fn title(&mut self, t: &str) -> AppResult<()> {
        self.ws.write_string_with_format(self.row, 0, t, &self.bold).map_err(xe)?;
        self.row += 1;
        Ok(())
    }
    fn header(&mut self, cols: &[&str]) -> AppResult<()> {
        for (i, c) in cols.iter().enumerate() {
            self.ws.write_string_with_format(self.row, i as u16, *c, &self.bold).map_err(xe)?;
            self.ws.set_column_width(i as u16, if i == 0 { 34 } else { 18 }).map_err(xe)?;
        }
        self.row += 1;
        Ok(())
    }
    fn cells(&mut self, cells: Vec<Cell>) -> AppResult<()> {
        for (i, c) in cells.into_iter().enumerate() {
            let col = i as u16;
            match c {
                Cell::S(s) => self.ws.write_string(self.row, col, s).map(|_| ()),
                Cell::Money(v) => self.ws.write_number_with_format(self.row, col, v, &self.money).map(|_| ()),
                Cell::Pct(Some(v)) => self.ws.write_number_with_format(self.row, col, v / 100.0, &self.pct).map(|_| ()),
                Cell::Num(Some(v)) => self.ws.write_number_with_format(self.row, col, v, &self.num).map(|_| ()),
                Cell::Pct(None) | Cell::Num(None) => Ok(()),
            }
            .map_err(xe)?;
        }
        self.row += 1;
        Ok(())
    }
}

enum Cell {
    S(String),
    Money(f64),
    Pct(Option<f64>),
    Num(Option<f64>),
}

pub async fn export(
    State(st): State<AppState>,
    _u: CurrentUser,
    Path(file): Path<String>,
    Query(q): Query<FilterQuery>,
) -> AppResult<impl IntoResponse> {
    let view = file.strip_suffix(".xlsx").unwrap_or(&file).to_string();
    let f = resolve(&st, &q).await?;
    let mut wb = Workbook::new();
    let money = Format::new().set_num_format("#,##0");
    let pct = Format::new().set_num_format("0.0%");
    let num = Format::new().set_num_format("#,##0.0");
    let bold = Format::new().set_bold();
    let category_name = match f.category {
        Some(id) => sqlx::query_scalar::<_, String>("SELECT name FROM categories WHERE id = ?").bind(id).fetch_optional(&st.db).await?,
        None => None,
    };
    let vendor_name = match f.vendor {
        Some(id) => sqlx::query_scalar::<_, String>("SELECT name FROM vendors WHERE id = ?").bind(id).fetch_optional(&st.db).await?,
        None => None,
    };
    let filter_text = format!(
        "Filter: {} s/d {} (pembanding {} s/d {}){}{}",
        f.from,
        f.to,
        f.prev_from,
        f.prev_to,
        f.category.map(|c| format!(", kategori {}", category_name.clone().unwrap_or_else(|| format!("id {c}")))).unwrap_or_default(),
        f.vendor.map(|v| format!(", vendor {}", vendor_name.clone().unwrap_or_else(|| format!("id {v}")))).unwrap_or_default()
    );
    let scope = Scope { category: f.category, vendor: f.vendor, commodity: None };
    match view.as_str() {
        "spend" => {
            let cats = queries::spend_by_category_compare(&st.db, f.from, f.to, f.prev_from, f.prev_to, scope).await?;
            let items = queries::top_items(&st.db, f.from, f.to, scope, 1000).await?;
            {
                let ws = wb.add_worksheet().set_name("Spend per kategori").map_err(xe)?;
                let mut s = Sheet { ws, row: 0, money: money.clone(), pct: pct.clone(), num: num.clone(), bold: bold.clone() };
                s.title("Tilik - Spend per kategori")?;
                s.title(&filter_text)?;
                s.header(&["Kategori", "Spend (Rp)", "Spend periode sebelumnya (Rp)", "Perubahan", "Porsi"])?;
                for c in &cats {
                    s.cells(vec![
                        Cell::S(c.name.clone()),
                        Cell::Money(c.spend),
                        Cell::Money(c.prev_spend),
                        Cell::Pct(c.change_pct),
                        Cell::Pct(Some(c.share_pct)),
                    ])?;
                }
            }
            let ws = wb.add_worksheet().set_name("Spend per item").map_err(xe)?;
            let mut s = Sheet { ws, row: 0, money, pct, num, bold };
            s.title(&filter_text)?;
            s.header(&["Item", "Kategori", "Spend (Rp)", "Qty (satuan dasar)", "Satuan dasar", "Harga rata-rata (Rp)", "Jumlah PO"])?;
            for i in &items {
                s.cells(vec![
                    Cell::S(i.item.clone()),
                    Cell::S(i.category.clone()),
                    Cell::Money(i.spend),
                    Cell::Num(Some(i.base_qty)),
                    Cell::S(i.base_unit.clone()),
                    Cell::Money(i.avg_price),
                    Cell::Num(Some(i.po_count as f64)),
                ])?;
            }
        }
        "commodities" => {
            let reference = reference_date(&st).await?;
            let settings = Settings::load(&st.db).await?;
            let rows = queries::commodities_table(&st.db, reference).await?;
            {
                let ws = wb.add_worksheet().set_name("Harga mingguan").map_err(xe)?;
                let mut s = Sheet { ws, row: 0, money: money.clone(), pct: pct.clone(), num: num.clone(), bold: bold.clone() };
                s.title(&format!("Tilik - Harga komoditas minggu {}", rows.first().map(|r| r.this_week.clone()).unwrap_or_default()))?;
                s.header(&["Komoditas", "Satuan", "Harga minggu ini (Rp)", "Harga minggu lalu (Rp)", "Perubahan", "Alert"])?;
                for r in &rows {
                    s.cells(vec![
                        Cell::S(r.name.clone()),
                        Cell::S(r.base_unit.clone()),
                        Cell::Num(r.price_this_week),
                        Cell::Num(r.price_last_week),
                        Cell::Pct(r.change_pct),
                        Cell::S(r.alert_message.clone().unwrap_or_default()),
                    ])?;
                }
            }
            let mut forecasts = Vec::new();
            {
                let ws = wb.add_worksheet().set_name("52 minggu").map_err(xe)?;
                let mut s = Sheet { ws, row: 0, money: money.clone(), pct: pct.clone(), num: num.clone(), bold: bold.clone() };
                s.header(&["Komoditas", "Minggu", "Mulai", "Harga (Rp/satuan dasar)", "Qty", "Perubahan vs minggu lalu", "Alert"])?;
                for r in &rows {
                    let d = queries::commodity_detail(&st.db, r.id, reference, 52, settings.anomaly()).await?;
                    forecasts.push((r.name.clone(), r.base_unit.clone(), d.forecast.clone()));
                    for w in d.weeks {
                        s.cells(vec![
                            Cell::S(r.name.clone()),
                            Cell::S(w.week),
                            Cell::S(w.week_start),
                            Cell::Num(w.price),
                            Cell::Num(Some(w.qty)),
                            Cell::Pct(w.change_pct),
                            Cell::S(if w.alert { "ya".into() } else { String::new() }),
                        ])?;
                    }
                }
            }
            let ws = wb.add_worksheet().set_name("Proyeksi 4 minggu").map_err(xe)?;
            let mut s = Sheet { ws, row: 0, money, pct, num, bold };
            s.title(&format!("Proyeksi harga: regresi linear {} minggu terakhir, pita = interval prediksi 95%", queries::FORECAST_WINDOW))?;
            s.header(&[
                "Komoditas",
                "Satuan",
                "Minggu",
                "Mulai",
                "Proyeksi (Rp)",
                "Batas bawah (Rp)",
                "Batas atas (Rp)",
                "Tren per minggu",
            ])?;
            for (name, unit, fc) in forecasts {
                let Some(fc) = fc else { continue };
                for p in fc.points {
                    s.cells(vec![
                        Cell::S(name.clone()),
                        Cell::S(unit.clone()),
                        Cell::S(p.week),
                        Cell::S(p.week_start),
                        Cell::Num(Some(p.price)),
                        Cell::Num(Some(p.lower)),
                        Cell::Num(Some(p.upper)),
                        Cell::Pct(Some(fc.slope_pct_per_week)),
                    ])?;
                }
            }
        }
        "vendors" => {
            let settings = Settings::load(&st.db).await?;
            let rows = queries::vendor_scorecard(&st.db, f.from, f.to, scope, settings.weights()).await?;
            let w = settings.weights();
            {
                let ws = wb.add_worksheet().set_name("Scorecard vendor").map_err(xe)?;
                let mut s = Sheet { ws, row: 0, money: money.clone(), pct: pct.clone(), num: num.clone(), bold: bold.clone() };
                s.title("Tilik - Scorecard vendor")?;
                s.title(&format!("{filter_text}. Bobot: ketepatan {}%, kualitas {}%, harga {}%", w.on_time, w.quality, w.price))?;
                s.header(&[
                    "Vendor",
                    "Skor total",
                    "Label",
                    "Spend (Rp)",
                    "Jumlah PO",
                    "PO terkirim",
                    "PO telat",
                    "Tepat waktu",
                    "Rata-rata kualitas (1-5)",
                    "Reject",
                    "Skor kualitas",
                    "Selisih harga vs pasar",
                    "Skor harga",
                ])?;
                for r in &rows {
                    s.cells(vec![
                        Cell::S(r.vendor.clone()),
                        Cell::Num(r.total),
                        Cell::S(r.label.clone()),
                        Cell::Money(r.spend),
                        Cell::Num(Some(r.po_count as f64)),
                        Cell::Num(Some(r.delivered as f64)),
                        Cell::Num(Some(r.late as f64)),
                        Cell::Pct(r.on_time_pct),
                        Cell::Num(r.avg_quality),
                        Cell::Pct(r.reject_pct),
                        Cell::Num(r.quality_score),
                        Cell::Pct(r.price_diff_pct),
                        Cell::Num(r.price_score),
                    ])?;
                }
            }
            let sp = settings.split_po();
            let groups = queries::split_po_candidates(&st.db, f.from, f.to, f.vendor, sp).await?;
            let ws = wb.add_worksheet().set_name("Kemungkinan split PO").map_err(xe)?;
            let mut s = Sheet { ws, row: 0, money, pct, num, bold };
            s.title(&format!(
                "Kemungkinan split PO: >= {} PO ke vendor yang sama di hari yang sama, tiap PO < {}, total >= batas",
                sp.min_count,
                crate::aggregates::fmt_rp(sp.approval_limit)
            ))?;
            s.header(&["Tanggal", "Vendor", "Jumlah PO", "Total (Rp)", "PO terbesar (Rp)", "Di atas batas", "Item berulang", "Nomor PO"])?;
            for g in &groups {
                s.cells(vec![
                    Cell::S(g.po_date.to_string()),
                    Cell::S(g.vendor.clone()),
                    Cell::Num(Some(g.po_count as f64)),
                    Cell::Money(g.total),
                    Cell::Money(g.largest_po),
                    Cell::Pct(Some(g.over_limit_pct)),
                    Cell::S(g.repeated_items.join(", ")),
                    Cell::S(g.pos.iter().map(|p| p.po_no.as_str()).collect::<Vec<_>>().join(", ")),
                ])?;
            }
        }
        _ => return Err(AppError::NotFound("Export tersedia untuk spend, commodities, vendors".into())),
    }
    let bytes = wb.save_to_buffer().map_err(xe)?;
    let name = format!("tilik-{view}-{}-{}.xlsx", f.from, f.to);
    Ok((
        [
            (header::CONTENT_TYPE, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\"")),
        ],
        bytes,
    ))
}
