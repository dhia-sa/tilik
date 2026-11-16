# SpendView

Dashboard web untuk **spend per kategori**, **harga komoditas mingguan + alert**, **scorecard vendor**, dan **Tanya Data** (AI yang menjawab pertanyaan dengan angka dari database).

Penugasan Rust + React · Batch 1 PT Agrinesia Raya · Kode tugas **RR-B1-03** · Dhia Salsabila Azhar

> Semua data di repo ini adalah **data dummy** buatan generator (`seed/generate.py`). Tidak ada data asli perusahaan.

---

## Cara menjalankan (3 perintah)

Prasyarat: **Rust 1.80+** (`rustup`), **Node.js 20+**. Di Windows, Rust membutuhkan *Visual Studio C++ Build Tools* (dipakai juga untuk meng-compile SQLite bawaan).

```bash
npm --prefix frontend ci
npm --prefix frontend run build
cargo run --release --manifest-path backend/Cargo.toml
```

Buka **http://127.0.0.1:8080**. Saat pertama kali jalan, aplikasi otomatis membuat database `data/spendview.db`, menjalankan migrasi, membuat akun demo, dan mengimpor data dummy 12 bulan (± 1 detik).

Alternatif satu perintah: `./run.sh` (macOS/Linux) atau `.\run.ps1` (Windows PowerShell).

File `.env` **opsional** — semua variabel punya nilai default (lihat `.env.example`). Untuk mulai dari database kosong, hapus folder `data/`.

### Akun demo

| Role | Email | Password | Akses |
|---|---|---|---|
| Viewer | `viewer@spendview.local` | `viewer123` | Semua dashboard, Tanya Data, export |
| Data Admin | `dataadmin@spendview.local` | `dataadmin123` | + Import, validasi & rollback, master data, threshold & bobot |
| Admin IT | `admin@spendview.local` | `admin123` | + Kelola user & role (Admin juga boleh semua fitur Data Admin) |

Halaman login juga punya tombol "Masuk sebagai …" untuk ketiga akun.

### AI (Claude CLI / Codex CLI)

Aplikasi **tidak memanggil API LLM langsung dan tidak butuh API key**. Backend menjalankan CLI yang sudah login di mesin lokal sebagai subprocess.

| `AI_PROVIDER` | Yang dijalankan |
|---|---|
| `claude` (default) | `claude -p --output-format json --json-schema '<schema>' --tools ""` — prompt dikirim lewat **stdin**, hasil diambil dari `structured_output`, `is_error` dicek |
| `codex` | `codex exec --skip-git-repo-check -s read-only --output-schema schema.json -o out.json -` — prompt lewat stdin, hasil dibaca dari `out.json` |
| `mock` | Tidak memanggil apa pun: pemilihan tool dengan aturan kata kunci + ringkasan template, struktur JSON sama |

- Saat start, backend mengecek `<CLI> --version`. Kalau CLI tidak ditemukan, otomatis memakai `mock` (log menampilkan peringatan) supaya app tetap bisa dicoba. Set `AI_STRICT=true` untuk menonaktifkan fallback ini.
- Path binary bisa diatur lewat `CLAUDE_BIN` / `CODEX_BIN`. Di Windows backend juga mencoba `claude.cmd` / `claude.exe`.
- Versi CLI yang dipakai saat pengembangan: **Claude Code 2.1.325**. Provider yang aktif terlihat di sidebar (mis. "AI: claude").

---

## Fitur

| Fitur | Status | Halaman |
|---|---|---|
| F1 Login & role (argon2, cookie HttpOnly, cek role di backend) | Selesai | Login |
| F2 Import CSV PO & penilaian vendor: preview → error per baris + alasan → konfirmasi, riwayat + rollback | Selesai | Import |
| F3 Overview: KPI vs periode sebelumnya, tren 12 bulan, 5 alert terbaru, filter global | Selesai | Overview |
| F4 Spend per kategori, drill-down (tren, top 10 item, top 5 vendor), dekomposisi volume vs harga, **Jelaskan dengan AI** | Selesai | Spend |
| F5 Harga komoditas mingguan (rata-rata tertimbang, Δ%, sparkline 12 minggu), detail 52 minggu + harga per vendor, alert otomatis | Selesai | Komoditas |
| F6 Scorecard vendor (40/35/25), label, vendor bermasalah, detail vendor, bobot bisa diatur & langsung dihitung ulang | Selesai | Vendor |
| F7 Tanya Data: 7 tool + `none`, JSON ber-schema, query terparameter, verifikasi angka di backend, riwayat & contoh pertanyaan | Selesai | Tanya Data |
| F8 Export Excel (.xlsx) Spend, Komoditas, Vendor sesuai filter aktif | Selesai | tombol "Export Excel" |
| Pengaturan: kategori, komoditas, mapping item, konversi satuan, threshold, bobot, user | Selesai | Pengaturan |
| Bonus: mode gelap | Selesai | tombol di sidebar |
| Bonus: ringkasan mingguan, proyeksi harga, deteksi split PO | Tidak dikerjakan (prioritas fitur wajib) | – |

---

## Skenario uji (kriteria selesai)

1. **Import 12 bulan < 10 detik** — data seed sudah diimpor otomatis sebagai batch #1 (lihat Import → Riwayat, kolom Durasi ± 0,8 dtk untuk 8.575 baris). Untuk mengulang manual: login Data Admin → Import → **Rollback batch #1** → upload `seed/po_12bulan.csv` → Preview → Lanjutkan import. (Mengimpor ulang tanpa rollback akan ditolak sebagai duplikat `po_no + item`.)
2. **CSV kotor** — upload `seed/po_kotor.csv`: 30 baris error dengan alasannya (tanggal salah, `1.234,50`, `Rp 315.000`, vendor kosong, satuan `drum`/`bal`/`lusin`/`ton`, satuan tidak cocok, qty ≤ 0, kategori/komoditas tidak dikenal, `delivered_date` sebelum `po_date`, …) dan 20 baris valid. Klik "Lanjutkan import 20 baris valid".
3. **Rollback** batch CSV kotor → KPI kembali persis seperti sebelum import (dicek juga di `cargo test`).
4. **Total spend satu bulan** — bandingkan grafik tren Overview (hover bulan) dengan `seed/expected_totals.csv`, mis. **Okt 2026 = Rp 12.758.110.664 (554 PO)**, Des 2025 = Rp 15.682.499.415. Rumus: `SUM(qty × unit_price)` berdasarkan `po_date`.
5. **Dekomposisi Bahan Baku** — Spend → Bahan Baku. Klik "Jelaskan dengan AI"; narasi hanya boleh memuat angka dari tabel dekomposisi (dicek backend).
6. **Komoditas** — Gula Pasir naik bertahap sejak pertengahan Agustus 2026 (+6% di minggu terakhir, 2026-W46). Tepung Terigu melonjak +16% **hanya di 2026-W37 (7–13 Sep 2026)** dan alert muncul di minggu tersebut.
7. **Vendor** — `PT Gula Rasa Nusantara` (sering telat), `CV Kemas Prima Plastik` (reject tinggi), `CV Aroma Bahan Kue` (harga ~22% di atas vendor lain) berlabel *Perlu perhatian / Bermasalah*. Ubah bobot (mis. 10/10/80) di halaman Vendor → peringkat langsung berubah.
8. **Tanya Data** — 6 pertanyaan contoh tersedia sebagai tombol; tool + parameter bisa dibuka di setiap jawaban.
9. **Di luar data** — "Harga emas hari ini?" → tool `none`, dijawab bahwa data tidak tersedia.
10. **Export** — halaman Vendor → Export Excel (ikut filter periode & kategori).
11. **`cargo test`** — `cd backend && cargo test` (39 test: normalisasi satuan, rata-rata tertimbang, dekomposisi, scorecard, anomali, validasi CSV, verifikasi angka AI, routing mock, plus test integrasi seed → import → rollback).

---

## Arsitektur

```
spendview/
├── backend/                 Rust 2021, axum 0.8 + tokio, sqlx (SQLite)
│   ├── migrations/          skema SQL, dijalankan otomatis saat start
│   └── src/
│       ├── main.rs          start server, seed, sajikan frontend/dist
│       ├── config.rs        env var
│       ├── db.rs            koneksi, migrasi, seed akun demo + master + data dummy
│       ├── auth.rs          login/logout/me, argon2, sesi cookie, extractor CurrentUser + cek role
│       ├── units.rs         normalisasi satuan (kg/liter/pcs), rata-rata tertimbang  [test]
│       ├── import/          parse & validasi CSV per baris, commit 1 transaksi, rollback  [test]
│       ├── aggregates.rs    agregat mingguan komoditas + pembuatan alert (dihitung ulang setelah import/rollback)
│       ├── analytics/       logika murni: period (minggu ISO), decomposition, anomaly, scorecard  [test]
│       ├── queries.rs       semua query analitik (dipakai dashboard, export, dan tool AI)
│       ├── ai/              trait AiProvider (ClaudeCli, CodexCli, MockAi), tools, verifikasi angka  [test]
│       └── routes/          handler HTTP: dashboard, imports, ask, export, settings
├── frontend/                React 19 + Vite + TypeScript, Tailwind v4, Recharts
│   └── src/pages/           Login, Overview, Spend, Komoditas, Vendor, Tanya Data, Import, Pengaturan
├── seed/                    generator data dummy + CSV hasil + master.json + expected_totals.csv
├── .env.example
└── README.md
```

Satu binary menyajikan API di `/api/*` dan file hasil build frontend (SPA fallback ke `index.html`). Untuk development frontend: `npm --prefix frontend run dev` (Vite mem-proxy `/api` ke port 8080).

### Endpoint utama

| Method | Path | Role |
|---|---|---|
| POST | `/api/auth/login`, `/api/auth/logout` · GET `/api/auth/me` | Semua |
| POST | `/api/imports/preview` (multipart: `kind`, `file`) · `/api/imports/{token}/commit` | Data Admin |
| GET / POST | `/api/imports` · `/api/imports/{id}/rollback` | Data Admin |
| GET | `/api/overview?from=&to=&category=&vendor=` | Semua |
| GET | `/api/spend/by-category` · `/api/spend/category/{id}` · POST `/api/spend/category/{id}/explain` | Semua |
| GET | `/api/commodities` · `/api/commodities/{id}` | Semua |
| GET | `/api/vendors/scorecard` · `/api/vendors/{id}` | Semua |
| POST / GET | `/api/ask` · `/api/ask/history` | Semua |
| GET | `/api/alerts` · `/api/meta` | Semua |
| GET | `/api/export/{spend|commodities|vendors}.xlsx` | Semua |
| GET / PUT | `/api/settings` + CRUD `/api/master/*` | Data Admin |
| POST / PUT / DELETE | `/api/users`, `/api/users/{id}` | Admin |

---

## Keputusan teknis

**Data & normalisasi**
- Harga disimpan per satuan asli (`unit_price`) **dan** per satuan dasar (`base_unit_price`). Konversi dicari dulu per item (mis. 1 karton Mentega = 15 kg, 1 karton Plastik Kemasan = 1.000 pcs), lalu global (sak 25/50 kg, jerigen 18 L, gram, liter, pcs, paket, trip). Konversi global hanya berlaku kalau satuan dasarnya sama dengan item, jadi "4 kg Bearing" ditolak.
- Angka CSV harus format mesin (`1234.5`). Format lokal `1.234,50`, `Rp 315.000`, dan pola ribuan ambigu `2.500` ditolak dengan alasan yang jelas, supaya tidak ada salah baca 1000×.
- Duplikat dicek dengan kunci `po_no + item` (dalam file dan terhadap database). Vendor baru dibuat otomatis; item baru dibuat kalau satuannya dikenal. Rollback juga menghapus vendor/item yang dibuat batch itu kalau tidak dipakai data lain.
- "Hari ini" untuk data dummy = tanggal PO terakhir (2026-11-14), supaya "minggu ini/bulan ini" tetap bermakna kapan pun reviewer menjalankannya. Bisa diganti lewat `REFERENCE_DATE`.

**Analitik**
- Harga komoditas mingguan = `SUM(qty × harga) / SUM(qty)` dalam satuan dasar, minggu ISO Senin–Minggu. Agregat disimpan di tabel `commodity_weekly` dan dihitung ulang setelah setiap import/rollback/perubahan master.
- Alert harga: Δ minggu ini vs minggu lalu (minggu berurutan) > threshold (default 5%), **atau** harga > rata-rata 12 minggu sebelumnya + 2σ. Tambahan: rule σ juga mensyaratkan harga minimal 2% di atas rata-rata (bisa diatur) supaya fluktuasi kecil yang kebetulan > 2σ tidak membanjiri alert. Penurunan harga tidak memicu alert.
- Alert vendor: ketepatan kirim bulanan < 70% (min. 5 PO terkirim) atau reject bulanan > 5%.
- Dekomposisi: rumus brief per item lalu dijumlah. Item yang hanya ada di salah satu periode tidak punya harga pembanding, jadi dicatat sebagai **efek item baru / item tidak dibeli lagi** sehingga `volume + harga + baru + hilang = spend₁ − spend₀` selalu tepat.
- Scorecard: ketepatan kirim = % PO terkirim dengan `delivered_date ≤ promised_date` (PO belum terkirim tidak dihitung). Kualitas = `(rata-rata skor − 1)/4 × 100 − % reject`. Harga = `80 − 4 × selisih%` terhadap rata-rata harga semua vendor untuk item yang sama (tertimbang spend), dibatasi 0–100 — harga sama dengan pasar = 80, 5% lebih murah = 100, 20% lebih mahal = 0. Komponen tanpa data dibobot ulang.
- KPI dibandingkan dengan periode sebelumnya yang panjangnya sama. Filter default: 3 bulan terakhir.

**AI (Tanya Data & Jelaskan)**
- AI tidak menulis SQL. Panggilan pertama: AI memilih `{ tool, params, reason }` lewat JSON schema (semua field wajib/nullable, `additionalProperties: false`, kompatibel dengan Claude & Codex). Rust memvalidasi ulang dengan serde + aturan per tool; kalau tidak valid, AI diminta memperbaiki maks. 2x dengan pesan error, lalu menjawab "belum bisa menjawab" + saran pertanyaan.
- Tool: `spend_by`, `commodity_price`, `vendor_ranking`, `price_change`, `top_items`, `alerts`, `kpi_summary`, `none`. Semua memakai query terparameter yang sama dengan dashboard.
- Panggilan kedua: AI merangkum tabel hasil (2–4 kalimat). Backend mengekstrak setiap angka di ringkasan (memahami format `1.234.567`, `1,2 miliar`, `16,0%`, dll.) dan mencocokkannya dengan angka di tabel sesuai pembulatannya. Kalau ada angka yang tidak ada di tabel, AI diminta memperbaiki sekali; kalau tetap gagal, ditampilkan ringkasan otomatis (template) beserta alasannya.
- Proses CLI: tanpa shell (`tokio::process::Command`), prompt lewat stdin, `--tools ""` / `-s read-only`, timeout 120 detik, maks. 2 proses paralel (`Semaphore`), retry maks. 2x, setiap panggilan dicatat di tabel `ai_calls` (prompt, output mentah, error, durasi, provider).

**Lainnya**
- Tidak ada `unwrap()` di jalur request: semua error diubah ke JSON `{ "error": "..." }` dengan status yang sesuai.
- Library: axum, tokio, sqlx, tower-http, argon2, csv, chrono, rust_xlsxwriter, tracing; frontend React 19, react-router, Recharts, Tailwind v4 (sesuai saran brief; polars tidak dipakai karena agregasi cukup dengan SQL).

## Data dummy

`python3 seed/generate.py` (deterministik, seed 42) membuat ulang semua file di `seed/`:

- `po_12bulan.csv` — 8.575 baris PO (6.450 nomor PO), 17 Nov 2025 – 14 Nov 2026 (52 minggu ISO), 27 vendor, 60 item, 6 kategori, 8 komoditas, satuan campuran.
- Pola: gula naik bertahap 3 bulan terakhir; lonjakan tepung di 2026-W37; volume naik menjelang Ramadan/Lebaran (Jan–Mar 2026) dan Desember; 3 vendor bermasalah.
- `penilaian_vendor_12bulan.csv` — penilaian bulanan 2025-11 s/d 2026-10.
- `po_kotor.csv` — 30 baris error + 20 baris valid.
- `master.json` — kategori, komoditas, item & konversi satuan. `expected_totals.csv` — total spend & jumlah PO per bulan untuk cek manual.

## Keterbatasan

- Bonus (ringkasan mingguan otomatis, proyeksi harga, deteksi split PO) belum dikerjakan.
- Jawaban Tanya Data bergantung pada CLI yang sudah login di mesin lokal; mode `mock` hanya memahami pola pertanyaan umum (kata kunci).
- Sesi disimpan di database dan berlaku 12 jam; belum ada rate limit login.
- Waktu di riwayat import ditampilkan dalam UTC (perhitungan minggu & tanggal data memakai tanggal lokal WIB).
