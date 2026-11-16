//! Pengaturan (threshold alert, bobot scorecard) yang disimpan di tabel `settings`.

use crate::analytics::anomaly::AnomalyParams;
use crate::analytics::scorecard::Weights;
use crate::error::AppResult;
use sqlx::SqlitePool;
use std::collections::HashMap;

pub const NUMERIC_KEYS: [(&str, f64, f64); 8] = [
    ("price_alert_threshold_pct", 0.0, 100.0),
    ("price_alert_sigma", 0.5, 10.0),
    ("price_alert_min_dev_pct", 0.0, 100.0),
    ("vendor_alert_on_time_pct", 0.0, 100.0),
    ("vendor_alert_reject_pct", 0.0, 100.0),
    ("weight_on_time", 0.0, 100.0),
    ("weight_quality", 0.0, 100.0),
    ("weight_price", 0.0, 100.0),
];

#[derive(Debug, Clone)]
pub struct Settings {
    pub raw: HashMap<String, String>,
}

impl Settings {
    pub async fn load(db: &SqlitePool) -> AppResult<Self> {
        let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings").fetch_all(db).await?;
        Ok(Settings { raw: rows.into_iter().collect() })
    }
    pub fn num(&self, key: &str, default: f64) -> f64 {
        self.raw.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
    }
    pub fn anomaly(&self) -> AnomalyParams {
        AnomalyParams {
            threshold_pct: self.num("price_alert_threshold_pct", 5.0),
            sigma: self.num("price_alert_sigma", 2.0),
            min_dev_pct: self.num("price_alert_min_dev_pct", 2.0),
        }
    }
    pub fn weights(&self) -> Weights {
        Weights {
            on_time: self.num("weight_on_time", 40.0),
            quality: self.num("weight_quality", 35.0),
            price: self.num("weight_price", 25.0),
        }
    }
}
