//! Dekomposisi perubahan spend: efek volume vs efek harga (price-volume analysis).
//!
//! Per item (qty dalam satuan dasar, harga = rata-rata tertimbang per satuan dasar):
//!   efek volume = (qty1 - qty0) * harga0
//!   efek harga  = (harga1 - harga0) * qty1
//!   total       = efek volume + efek harga = spend1 - spend0
//! Item yang hanya muncul di salah satu periode tidak punya harga pembanding,
//! sehingga dicatat terpisah sebagai "item baru" / "item tidak dibeli lagi"
//! agar total tetap sama dengan selisih spend.

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ItemPeriods {
    pub item_id: i64,
    pub name: String,
    pub qty0: f64,
    pub spend0: f64,
    pub qty1: f64,
    pub spend1: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemEffect {
    pub item_id: i64,
    pub name: String,
    pub spend0: f64,
    pub spend1: f64,
    pub qty0: f64,
    pub qty1: f64,
    pub price0: Option<f64>,
    pub price1: Option<f64>,
    pub volume_effect: f64,
    pub price_effect: f64,
    pub new_item_effect: f64,
    pub lost_item_effect: f64,
    pub total_change: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Decomposition {
    pub spend0: f64,
    pub spend1: f64,
    pub volume_effect: f64,
    pub price_effect: f64,
    pub new_item_effect: f64,
    pub lost_item_effect: f64,
    pub total_change: f64,
    pub items: Vec<ItemEffect>,
}

pub fn decompose_item(r: &ItemPeriods) -> ItemEffect {
    let price0 = if r.qty0 > 0.0 { Some(r.spend0 / r.qty0) } else { None };
    let price1 = if r.qty1 > 0.0 { Some(r.spend1 / r.qty1) } else { None };
    let (mut vol, mut price, mut new, mut lost) = (0.0, 0.0, 0.0, 0.0);
    match (price0, price1) {
        (Some(p0), Some(p1)) => {
            vol = (r.qty1 - r.qty0) * p0;
            price = (p1 - p0) * r.qty1;
        }
        (None, Some(_)) => new = r.spend1,
        (Some(_), None) => lost = -r.spend0,
        (None, None) => {}
    }
    ItemEffect {
        item_id: r.item_id,
        name: r.name.clone(),
        spend0: r.spend0,
        spend1: r.spend1,
        qty0: r.qty0,
        qty1: r.qty1,
        price0,
        price1,
        volume_effect: vol,
        price_effect: price,
        new_item_effect: new,
        lost_item_effect: lost,
        total_change: vol + price + new + lost,
    }
}

pub fn decompose(rows: &[ItemPeriods]) -> Decomposition {
    let mut out = Decomposition::default();
    for r in rows {
        let e = decompose_item(r);
        out.spend0 += e.spend0;
        out.spend1 += e.spend1;
        out.volume_effect += e.volume_effect;
        out.price_effect += e.price_effect;
        out.new_item_effect += e.new_item_effect;
        out.lost_item_effect += e.lost_item_effect;
        out.total_change += e.total_change;
        out.items.push(e);
    }
    out.items.sort_by(|a, b| b.total_change.abs().total_cmp(&a.total_change.abs()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, qty0: f64, p0: f64, qty1: f64, p1: f64) -> ItemPeriods {
        ItemPeriods { item_id: id, name: format!("item{id}"), qty0, spend0: qty0 * p0, qty1, spend1: qty1 * p1 }
    }

    #[test]
    fn rumus_volume_dan_harga() {
        // gula: 1.000 kg @14.000 -> 1.200 kg @15.000
        let e = decompose_item(&row(1, 1000.0, 14_000.0, 1200.0, 15_000.0));
        assert!((e.volume_effect - 200.0 * 14_000.0).abs() < 1e-6); // 2.800.000
        assert!((e.price_effect - 1_000.0 * 1200.0).abs() < 1e-6); // 1.200.000
        assert!((e.total_change - (18_000_000.0 - 14_000_000.0)).abs() < 1e-6);
    }

    #[test]
    fn total_sama_dengan_selisih_spend() {
        let rows = vec![
            row(1, 1000.0, 14_000.0, 1200.0, 15_000.0),
            row(2, 500.0, 9_800.0, 400.0, 11_000.0),
            row(3, 0.0, 0.0, 50.0, 85_000.0), // item baru
            row(4, 20.0, 150_000.0, 0.0, 0.0), // tidak dibeli lagi
        ];
        let d = decompose(&rows);
        assert!((d.total_change - (d.spend1 - d.spend0)).abs() < 1e-6);
        assert!((d.new_item_effect - 4_250_000.0).abs() < 1e-6);
        assert!((d.lost_item_effect + 3_000_000.0).abs() < 1e-6);
        assert!(((d.volume_effect + d.price_effect + d.new_item_effect + d.lost_item_effect) - d.total_change).abs() < 1e-6);
    }

    #[test]
    fn harga_turun_volume_naik() {
        let e = decompose_item(&row(1, 100.0, 10_000.0, 150.0, 9_000.0));
        assert!((e.volume_effect - 500_000.0).abs() < 1e-6);
        assert!((e.price_effect + 150_000.0).abs() < 1e-6);
    }
}
