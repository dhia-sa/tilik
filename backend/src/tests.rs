//! Test integrasi: seed -> import -> agregat -> rollback memakai database SQLite sementara.

use crate::config::Config;
use crate::import::{self, ImportKind, PendingImport};
use crate::queries::{self, Scope};
use chrono::NaiveDate;
use std::time::Instant;

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("tanggal valid")
}

async fn setup() -> (sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut cfg = Config::from_env();
    cfg.database_url = format!("sqlite://{}?mode=rwc", dir.path().join("t.db").to_string_lossy().replace('\\', "/"));
    cfg.seed_on_start = true;
    let pool = crate::db::connect(&cfg).await.expect("connect");
    crate::db::seed(&pool, &cfg).await.expect("seed");
    (pool, dir)
}

#[tokio::test]
async fn seed_total_bulanan_cocok_dengan_expected_totals() {
    let (pool, _dir) = setup().await;
    let expected = std::fs::read_to_string(Config::from_env().seed_dir.join("expected_totals.csv")).expect("expected_totals.csv");
    for line in expected.lines().skip(1) {
        let parts: Vec<&str> = line.split(',').collect();
        let month = parts[0];
        let total: f64 = parts[1].parse().expect("angka");
        let pos: i64 = parts[2].parse().expect("angka");
        let from = d(&format!("{month}-01"));
        let to = crate::analytics::period::month_end(from);
        let k = queries::kpis(&pool, from, to, Scope::default()).await.expect("kpi");
        assert!((k.total_spend - total).abs() < 0.5, "{month}: {} vs {total}", k.total_spend);
        assert_eq!(k.po_count, pos, "{month}");
    }
}

#[tokio::test]
async fn import_kotor_lalu_rollback_mengembalikan_data() {
    let (pool, _dir) = setup().await;
    let all = (d("2025-01-01"), d("2027-12-31"));
    let before = queries::kpis(&pool, all.0, all.1, Scope::default()).await.expect("kpi");
    let data = std::fs::read(Config::from_env().seed_dir.join("po_kotor.csv")).expect("po_kotor.csv");
    let master = import::load_master(&pool).await.expect("master");
    let v = import::validate(ImportKind::Po, &data, &master).expect("validasi");
    assert_eq!(v.errors.len(), 30);
    assert_eq!(v.rows.len(), 20);
    let p = PendingImport { kind: ImportKind::Po, file_name: "po_kotor.csv".into(), user_id: 2, rows: v.rows, errors: 30, created: Instant::now() };
    let r = import::commit(&pool, &p).await.expect("commit");
    let mid = queries::kpis(&pool, all.0, all.1, Scope::default()).await.expect("kpi");
    assert_eq!(mid.po_count, before.po_count + 20);
    import::rollback(&pool, r.batch_id).await.expect("rollback");
    let after = queries::kpis(&pool, all.0, all.1, Scope::default()).await.expect("kpi");
    assert_eq!(after.po_count, before.po_count);
    assert!((after.total_spend - before.total_spend).abs() < 0.5);
    assert!(import::rollback(&pool, r.batch_id).await.is_err(), "rollback dua kali harus ditolak");
}

#[tokio::test]
async fn pola_data_dummy_terdeteksi() {
    let (pool, _dir) = setup().await;
    // lonjakan tepung memicu alert di 2026-W37
    let alerts = queries::alerts(&pool, Some("price"), None, None, 500).await.expect("alerts");
    assert!(alerts.iter().any(|a| a.ref_name == "Tepung Terigu" && a.week == "2026-W37"));
    assert!(!alerts.iter().any(|a| a.ref_name == "Tepung Terigu" && a.week != "2026-W37"));
    // 3 vendor bermasalah tidak berlabel Baik
    let rows = queries::vendor_scorecard(&pool, d("2025-11-17"), d("2026-11-14"), Scope::default(), Default::default()).await.expect("scorecard");
    for v in ["PT Gula Rasa Nusantara", "CV Kemas Prima Plastik", "CV Aroma Bahan Kue"] {
        let r = rows.iter().find(|r| r.vendor == v).expect("vendor ada");
        assert_ne!(r.label, "Baik", "{v} harus bermasalah/perlu perhatian");
    }
    assert!(rows.iter().filter(|r| r.label != "Baik").count() == 3);
}
