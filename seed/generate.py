#!/usr/bin/env python3
"""
Generator data dummy SpendView (deterministik, seed=42, tanpa dependency).

Menghasilkan di folder seed/:
  - master.json                 : kategori, komoditas, item, konversi satuan
  - po_12bulan.csv              : 12 bulan data PO (17 Nov 2025 - 15 Nov 2026, 52 minggu ISO)
  - penilaian_vendor_12bulan.csv: penilaian vendor bulanan (2025-11 .. 2026-10)
  - po_kotor.csv                : file CSV "kotor" untuk demo validasi (~30 baris error)
  - po_split_demo.csv           : 4 PO kecil ke vendor yang sama di hari yang sama (demo deteksi split PO)
  - expected_totals.csv         : total spend per bulan (untuk cek manual reviewer)

Pola yang sengaja dibuat:
  - Gula naik bertahap 3 bulan terakhir (+~1.3%/minggu) dan loncat +6% di minggu terakhir (2026-W46).
  - Tepung terigu melonjak +16% hanya di minggu 2026-W37 (7-13 Sep 2026).
  - Musiman: volume naik menjelang Ramadan/Lebaran (Jan - pertengahan Mar 2026) dan akhir tahun (Des).
  - 3 vendor bermasalah:
      PT Gula Rasa Nusantara  -> sering telat kirim
      CV Kemas Prima Plastik  -> reject tinggi + skor kualitas rendah
      CV Aroma Bahan Kue      -> harga ~22% di atas rata-rata pasar internal
  - Satuan campuran: kg, gram, sak 25 kg, sak 50 kg, karton, liter, jerigen 18 L, pcs, paket, trip.

Semua nama vendor/item adalah fiktif.
"""
import csv
import json
import math
import os
import random
from collections import defaultdict
from datetime import date, timedelta

random.seed(42)
HERE = os.path.dirname(os.path.abspath(__file__))

START = date(2025, 11, 17)  # Senin, 2025-W47
END = date(2026, 11, 15)    # Minggu, 2026-W46
WEEKS = 52

CATEGORIES = ["Bahan Baku", "Kemasan", "Sparepart", "Jasa", "Logistik", "Bahan Pendukung"]

COMMODITIES = [
    ("Gula Pasir", "kg"),
    ("Tepung Terigu", "kg"),
    ("Minyak Nabati", "liter"),
    ("Mentega", "kg"),
    ("Susu Bubuk", "kg"),
    ("Cokelat Bubuk", "kg"),
    ("Keju", "kg"),
    ("Telur Ayam", "kg"),
]

GLOBAL_UNITS = [
    # (from_unit, base_unit, to_base)
    ("kg", "kg", 1), ("gram", "kg", 0.001), ("sak 25 kg", "kg", 25), ("sak 50 kg", "kg", 50),
    ("liter", "liter", 1), ("jerigen 18 l", "liter", 18),
    ("pcs", "pcs", 1), ("paket", "pcs", 1), ("trip", "pcs", 1),
]

V = {
    "manis": "PT Manis Lestari Abadi", "gularasa": "PT Gula Rasa Nusantara", "kristal": "CV Kristal Tebu Jaya",
    "tepung": "PT Tepung Mulia Sentosa", "gandum": "PT Gandum Emas Raya", "butir": "CV Butir Halus Mandiri",
    "sawit": "PT Sawit Cahaya Pangan", "bening": "CV Minyak Bening Sejati",
    "susu": "PT Susu Segar Prima", "dairy": "PT Dairy Nusa Pratama", "lumbung": "CV Lumbung Susu",
    "kakao": "PT Kakao Lestari Jaya", "telur": "CV Peternakan Telur Makmur", "berkah": "UD Telur Berkah",
    "aroma": "CV Aroma Bahan Kue", "roti": "PT Bahan Roti Sejahtera",
    "kemas": "CV Kemas Prima Plastik", "karton": "PT Karton Indah Perkasa", "plastindo": "PT Plastindo Kemas Utama",
    "teknik": "PT Teknik Mesin Andalan", "spare": "CV Sparepart Bakery Jaya",
    "servis": "PT Solusi Servis Industri", "higienis": "CV Bersih Higienis",
    "ekspedisi": "PT Ekspedisi Cepat Lancar", "armada": "PT Armada Dingin Logistik",
    "k3": "CV Sarana K3 Mandiri", "gas": "PT Gas Energi Abadi",
}
LATE_VENDOR = V["gularasa"]
REJECT_VENDOR = V["kemas"]
PRICEY_VENDOR = V["aroma"]

# vendor price factor (vs harga pasar)
VENDOR_FACTOR = defaultdict(lambda: 1.0)
for k in V.values():
    VENDOR_FACTOR[k] = round(random.uniform(0.985, 1.015), 4)
VENDOR_FACTOR[PRICEY_VENDOR] = 1.22

# item: name, category, commodity, base_unit, base_price (Rp/base unit), units [(unit, weight)],
#       base qty range per line, lambda (line/minggu), vendors, item-specific karton size
ITEMS = [
    # --- Bahan Baku (komoditas)
    ("Gula Pasir Kristal Putih", "Bahan Baku", "Gula Pasir", "kg", 14500, [("sak 50 kg", 0.7), ("kg", 0.3)], (1500, 5000), 4.0, ["manis", "gularasa", "kristal"], None),
    ("Gula Halus", "Bahan Baku", "Gula Pasir", "kg", 14900, [("sak 25 kg", 0.6), ("kg", 0.4)], (300, 1200), 2.0, ["manis", "gularasa"], None),
    ("Tepung Terigu Protein Tinggi", "Bahan Baku", "Tepung Terigu", "kg", 9800, [("sak 25 kg", 1.0)], (2000, 8000), 4.0, ["tepung", "gandum", "butir"], None),
    ("Tepung Terigu Protein Sedang", "Bahan Baku", "Tepung Terigu", "kg", 9600, [("sak 25 kg", 1.0)], (1000, 5000), 3.0, ["tepung", "gandum"], None),
    ("Minyak Goreng Sawit", "Bahan Baku", "Minyak Nabati", "liter", 17500, [("jerigen 18 l", 0.7), ("liter", 0.3)], (500, 2000), 3.0, ["sawit", "bening"], None),
    ("Mentega Tawar", "Bahan Baku", "Mentega", "kg", 52000, [("karton", 0.8), ("kg", 0.2)], (300, 1200), 2.5, ["susu", "dairy", "lumbung"], 15),
    ("Susu Bubuk Full Cream", "Bahan Baku", "Susu Bubuk", "kg", 58000, [("sak 25 kg", 1.0)], (250, 1000), 2.5, ["susu", "dairy", "lumbung"], None),
    ("Cokelat Bubuk", "Bahan Baku", "Cokelat Bubuk", "kg", 85000, [("kg", 0.7), ("sak 25 kg", 0.3)], (100, 500), 2.0, ["kakao", "butir"], None),
    ("Keju Cheddar Blok", "Bahan Baku", "Keju", "kg", 110000, [("karton", 0.8), ("kg", 0.2)], (100, 400), 2.0, ["susu", "dairy"], 10),
    ("Telur Ayam Negeri", "Bahan Baku", "Telur Ayam", "kg", 28000, [("kg", 1.0)], (300, 1000), 4.0, ["telur", "berkah"], None),
    # --- Bahan Baku (non komoditas)
    ("Ragi Instan", "Bahan Baku", None, "kg", 62000, [("kg", 0.6), ("gram", 0.4)], (30, 150), 2.0, ["aroma", "roti", "kristal"], None),
    ("Garam Halus", "Bahan Baku", None, "kg", 6500, [("sak 25 kg", 0.5), ("kg", 0.5)], (100, 500), 1.5, ["aroma", "roti"], None),
    ("Baking Powder", "Bahan Baku", None, "kg", 45000, [("kg", 1.0)], (20, 100), 1.5, ["aroma", "roti"], None),
    ("Vanili Bubuk", "Bahan Baku", None, "kg", 150000, [("kg", 0.5), ("gram", 0.5)], (5, 25), 1.0, ["aroma", "roti"], None),
    ("Pewarna Makanan", "Bahan Baku", None, "liter", 120000, [("liter", 1.0)], (5, 20), 1.0, ["aroma", "roti"], None),
    ("Selai Stroberi", "Bahan Baku", None, "kg", 41000, [("kg", 1.0)], (100, 400), 1.5, ["aroma", "roti", "kristal"], None),
    ("Kismis", "Bahan Baku", None, "kg", 56000, [("kg", 1.0)], (50, 250), 1.0, ["aroma", "roti"], None),
    ("Wijen Putih", "Bahan Baku", None, "kg", 47000, [("kg", 1.0)], (30, 150), 1.0, ["aroma", "roti"], None),
    # --- Kemasan
    ("Plastik Kemasan Roti", "Kemasan", None, "pcs", 350, [("karton", 0.8), ("pcs", 0.2)], (20000, 80000), 3.0, ["kemas", "plastindo"], 1000),
    ("Box Kue Karton", "Kemasan", None, "pcs", 2500, [("karton", 1.0)], (2000, 8000), 2.0, ["karton", "kemas"], 100),
    ("Paper Cup 8oz", "Kemasan", None, "pcs", 600, [("karton", 1.0)], (5000, 20000), 1.5, ["karton", "plastindo"], 1000),
    ("Label Stiker Produk", "Kemasan", None, "pcs", 55, [("pcs", 1.0)], (20000, 100000), 1.5, ["kemas", "karton"], None),
    ("Tray Plastik", "Kemasan", None, "pcs", 1200, [("karton", 1.0)], (2000, 10000), 1.5, ["kemas", "plastindo"], 200),
    ("Kantong Kresek", "Kemasan", None, "kg", 35000, [("kg", 1.0)], (50, 300), 1.5, ["kemas", "plastindo"], None),
    ("Lakban Bening", "Kemasan", None, "pcs", 12000, [("pcs", 1.0)], (50, 300), 1.0, ["karton", "plastindo"], None),
    ("Stretch Film", "Kemasan", None, "pcs", 95000, [("pcs", 1.0)], (10, 60), 1.0, ["plastindo", "kemas"], None),
    ("Dus Karton Besar", "Kemasan", None, "pcs", 6500, [("pcs", 1.0)], (500, 3000), 1.5, ["karton"], None),
    ("Plastik Vakum", "Kemasan", None, "pcs", 800, [("karton", 1.0)], (5000, 20000), 1.0, ["kemas", "plastindo"], 500),
    ("Mika Kue", "Kemasan", None, "pcs", 900, [("karton", 1.0)], (3000, 15000), 1.5, ["kemas", "karton"], 500),
    ("Pita Kemasan", "Kemasan", None, "pcs", 15000, [("pcs", 1.0)], (20, 100), 0.5, ["karton"], None),
    # --- Sparepart
    ("Bearing 6205", "Sparepart", None, "pcs", 85000, [("pcs", 1.0)], (4, 20), 0.6, ["teknik", "spare"], None),
    ("V-Belt Mixer", "Sparepart", None, "pcs", 120000, [("pcs", 1.0)], (2, 10), 0.5, ["teknik", "spare"], None),
    ("Seal Pintu Oven", "Sparepart", None, "pcs", 350000, [("pcs", 1.0)], (1, 6), 0.4, ["teknik", "spare"], None),
    ("Heater Element Oven", "Sparepart", None, "pcs", 650000, [("pcs", 1.0)], (1, 6), 0.4, ["teknik", "spare"], None),
    ("Sensor Suhu", "Sparepart", None, "pcs", 1200000, [("pcs", 1.0)], (1, 4), 0.3, ["teknik"], None),
    ("Carbon Brush Motor Mixer", "Sparepart", None, "pcs", 250000, [("pcs", 1.0)], (2, 10), 0.4, ["spare", "teknik"], None),
    ("Conveyor Belt", "Sparepart", None, "pcs", 3500000, [("pcs", 1.0)], (1, 2), 0.15, ["teknik"], None),
    ("Filter Udara Kompresor", "Sparepart", None, "pcs", 450000, [("pcs", 1.0)], (1, 6), 0.4, ["spare", "teknik"], None),
    ("Oli Gearbox", "Sparepart", None, "liter", 95000, [("liter", 0.5), ("jerigen 18 l", 0.5)], (18, 72), 0.4, ["spare", "teknik"], None),
    ("Nozzle Filling", "Sparepart", None, "pcs", 180000, [("pcs", 1.0)], (2, 8), 0.3, ["spare"], None),
    # --- Jasa
    ("Servis Mesin Oven", "Jasa", None, "pcs", 8000000, [("paket", 1.0)], (1, 1), 0.35, ["servis"], None),
    ("Kalibrasi Timbangan", "Jasa", None, "pcs", 2500000, [("paket", 1.0)], (1, 2), 0.25, ["servis"], None),
    ("Pest Control Bulanan", "Jasa", None, "pcs", 4500000, [("paket", 1.0)], (1, 1), 0.23, ["higienis"], None),
    ("Cleaning Service Pabrik", "Jasa", None, "pcs", 15000000, [("paket", 1.0)], (1, 1), 0.23, ["higienis"], None),
    ("Maintenance AC", "Jasa", None, "pcs", 3000000, [("paket", 1.0)], (1, 2), 0.25, ["servis", "higienis"], None),
    ("Uji Lab Produk", "Jasa", None, "pcs", 2000000, [("paket", 1.0)], (1, 3), 0.3, ["servis"], None),
    # --- Logistik
    ("Pengiriman Jakarta-Bandung", "Logistik", None, "pcs", 2800000, [("trip", 1.0)], (1, 4), 2.0, ["ekspedisi", "armada"], None),
    ("Pengiriman Jakarta-Surabaya", "Logistik", None, "pcs", 7500000, [("trip", 1.0)], (1, 3), 1.2, ["ekspedisi", "armada"], None),
    ("Pengiriman Dalam Kota", "Logistik", None, "pcs", 850000, [("trip", 1.0)], (2, 10), 3.0, ["ekspedisi"], None),
    ("Sewa Truk Reefer Harian", "Logistik", None, "pcs", 3200000, [("trip", 1.0)], (1, 3), 1.0, ["armada"], None),
    ("Ekspedisi Laut Makassar", "Logistik", None, "pcs", 12000000, [("trip", 1.0)], (1, 2), 0.3, ["ekspedisi"], None),
    ("Kurir Dokumen", "Logistik", None, "pcs", 50000, [("pcs", 1.0)], (5, 30), 1.0, ["ekspedisi"], None),
    # --- Bahan Pendukung
    ("Gas LPG 50 kg", "Bahan Pendukung", None, "pcs", 950000, [("pcs", 1.0)], (5, 30), 2.0, ["gas"], None),
    ("Sabun Cuci Tangan", "Bahan Pendukung", None, "liter", 35000, [("jerigen 18 l", 0.6), ("liter", 0.4)], (18, 108), 1.0, ["k3", "higienis"], None),
    ("Sarung Tangan Plastik", "Bahan Pendukung", None, "pcs", 400, [("karton", 1.0)], (2000, 10000), 1.0, ["k3"], 1000),
    ("Hairnet", "Bahan Pendukung", None, "pcs", 300, [("karton", 1.0)], (2000, 10000), 1.0, ["k3"], 1000),
    ("Masker Medis", "Bahan Pendukung", None, "pcs", 250, [("karton", 1.0)], (4000, 20000), 0.8, ["k3"], 2000),
    ("Alkohol 70%", "Bahan Pendukung", None, "liter", 30000, [("jerigen 18 l", 0.7), ("liter", 0.3)], (18, 108), 0.8, ["k3", "higienis"], None),
    ("Tisu Gulung", "Bahan Pendukung", None, "pcs", 18000, [("karton", 1.0)], (96, 480), 0.8, ["k3"], 48),
    ("Desinfektan", "Bahan Pendukung", None, "liter", 45000, [("jerigen 18 l", 0.6), ("liter", 0.4)], (18, 90), 0.6, ["k3", "higienis"], None),
]

LEAD_DAYS = {"Bahan Baku": (2, 5), "Kemasan": (5, 10), "Sparepart": (5, 14), "Jasa": (3, 7), "Logistik": (1, 2), "Bahan Pendukung": (3, 7)}


def iso_week(d):
    y, w, _ = d.isocalendar()
    return f"{y}-W{w:02d}"


def week_index(d):
    return (d - START).days // 7  # 0..51


def commodity_index(commodity, d):
    """Indeks harga komoditas (1.0 = normal) untuk tanggal d."""
    wi = week_index(d)
    if commodity == "Gula Pasir":
        rise_start = 39  # minggu ke-39 (16 Agu 2026) -> 3 bulan terakhir
        idx = 1.0
        if wi >= rise_start:
            idx *= 1.0 + 0.013 * (wi - rise_start + 1)
        if wi == WEEKS - 1:  # minggu terakhir loncat +6%
            idx *= 1.06
        return idx
    if commodity == "Tepung Terigu":
        return 1.16 if iso_week(d) == "2026-W37" else 1.0
    if commodity in ("Mentega", "Susu Bubuk", "Keju"):
        # sedikit naik saat permintaan Ramadan (gradual, < 5%/minggu)
        peak = date(2026, 3, 1)
        dist = abs((d - peak).days)
        return 1.0 + max(0.0, 0.04 * (1 - dist / 60))
    return 1.0


def season_factor(d, category):
    f = 1.0
    if d.month == 12:
        f *= 1.25  # akhir tahun (Natal & Tahun Baru)
    if date(2026, 1, 15) <= d <= date(2026, 3, 15) and category in ("Bahan Baku", "Kemasan", "Logistik"):
        f *= 1.35  # menjelang Ramadan / Lebaran (Idulfitri ~20 Mar 2026)
    if date(2026, 3, 18) <= d <= date(2026, 3, 28):
        f *= 0.5  # libur Lebaran
    return f


def poisson(lam):
    L = math.exp(-lam)
    k, p = 0, 1.0
    while True:
        p *= random.random()
        if p <= L:
            return k
        k += 1


def unit_size(unit, karton):
    sizes = {"kg": 1, "gram": 0.001, "sak 25 kg": 25, "sak 50 kg": 50, "liter": 1, "jerigen 18 l": 18,
             "pcs": 1, "paket": 1, "trip": 1}
    if unit == "karton":
        return karton
    return sizes[unit]


UNIT_DISPLAY = {"jerigen 18 l": "jerigen 18 L"}


def pick(weighted):
    r = random.random() * sum(w for _, w in weighted)
    for v, w in weighted:
        r -= w
        if r <= 0:
            return v
    return weighted[-1][0]


def gen_lines():
    lines = []
    d = START
    while d <= END:
        weekday = d.weekday()
        workday = weekday < 6  # Senin-Sabtu
        if workday:
            for (name, cat, com, base, price, units, qrange, lam, vendors, karton) in ITEMS:
                n = poisson(lam * 2.0 / 6.0 * season_factor(d, cat))
                for _ in range(n):
                    vkey = random.choice(vendors)
                    vendor = V[vkey]
                    unit = pick(units)
                    size = unit_size(unit, karton)
                    base_qty_target = random.uniform(*qrange) * (season_factor(d, cat) ** 0.3)
                    qty = max(1, round(base_qty_target / size))
                    if unit == "gram":
                        qty = max(500, round(base_qty_target * 1000 / 500) * 500)
                    base_qty = qty * size
                    p_base = price * (commodity_index(com, d) if com else 1.0) * VENDOR_FACTOR[vendor] * (1 + random.gauss(0, 0.006))
                    unit_price = p_base * size
                    unit_price = int(round(unit_price / 10) * 10) if unit_price >= 100 else max(1, int(round(unit_price)))
                    lo, hi = LEAD_DAYS[cat]
                    promised = d + timedelta(days=random.randint(lo, hi))
                    if vendor == LATE_VENDOR:
                        late = random.random() < 0.55
                    else:
                        late = random.random() < 0.07
                    delivered = promised + timedelta(days=random.randint(1, 4)) if late else promised - timedelta(days=random.choice([0, 0, 0, 1]))
                    if delivered < d:
                        delivered = d
                    lines.append({
                        "po_date": d, "vendor": vendor, "category": cat, "item": name, "commodity": com or "",
                        "qty": qty, "unit": UNIT_DISPLAY.get(unit, unit), "unit_price": unit_price,
                        "promised_date": promised, "delivered_date": delivered if delivered <= END else None,
                        "base_qty": base_qty,
                    })
        d += timedelta(days=1)
    # Kelompokkan baris ke nomor PO: vendor + tanggal yang sama, maksimal 3 baris per PO
    lines.sort(key=lambda r: (r["po_date"], r["vendor"], r["item"]))
    counters = defaultdict(int)
    i = 0
    while i < len(lines):
        r = lines[i]
        group = [r]
        j = i + 1
        while (j < len(lines) and len(group) < 3 and lines[j]["po_date"] == r["po_date"] and lines[j]["vendor"] == r["vendor"]
               and lines[j]["item"] not in {g["item"] for g in group}):
            group.append(lines[j])
            j += 1
        ym = r["po_date"].strftime("%y%m")
        counters[ym] += 1
        po_no = f"PO-{ym}-{counters[ym]:05d}"
        # PO dalam satu grup memakai promised/delivered yang sama
        for g in group:
            g["po_no"] = po_no
            g["promised_date"] = group[0]["promised_date"]
            g["delivered_date"] = group[0]["delivered_date"]
        i = j
    return lines


def fmt(d):
    return d.isoformat() if d else ""


def write_po(lines, path):
    with open(path, "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["po_no", "po_date", "vendor", "category", "item", "commodity", "qty", "unit", "unit_price", "delivered_date", "promised_date"])
        for r in lines:
            w.writerow([r["po_no"], fmt(r["po_date"]), r["vendor"], r["category"], r["item"], r["commodity"], r["qty"], r["unit"], r["unit_price"], fmt(r["delivered_date"]), fmt(r["promised_date"])])


def gen_ratings(lines):
    by = defaultdict(float)
    for r in lines:
        if r["delivered_date"] is None:
            continue
        period = r["delivered_date"].strftime("%Y-%m")
        by[(period, r["vendor"])] += r["base_qty"]
    periods = []
    y, m = 2025, 11
    for _ in range(12):
        periods.append(f"{y}-{m:02d}")
        m += 1
        if m > 12:
            y, m = y + 1, 1
    good_fb = ["Pengiriman rapi, dokumen lengkap.", "Kualitas konsisten.", "Respon cepat saat ada komplain.", "Tidak ada kendala bulan ini.", "Kemasan barang baik."]
    bad_fb_reject = ["Banyak kemasan sobek, perlu retur.", "Ketebalan plastik tidak sesuai spesifikasi.", "Cetakan buram, sebagian ditolak QC.", "Reject tinggi, sudah ditegur."]
    late_fb = ["Pengiriman sering mundur 2-3 hari.", "Barang datang terlambat, produksi tertunda.", "Kualitas oke tapi jadwal kirim tidak tepat."]
    rows = []
    for p in periods:
        for vendor in sorted(set(v for (pp, v) in by if pp == p)):
            total = by[(p, vendor)]
            if vendor == REJECT_VENDOR:
                q = min(5, max(1, random.gauss(2.6, 0.25)))
                rej = random.uniform(0.10, 0.16)
                fb = random.choice(bad_fb_reject)
            else:
                q = min(5, max(1, random.gauss(4.3, 0.25)))
                rej = random.uniform(0.003, 0.02)
                fb = random.choice(late_fb if vendor == LATE_VENDOR else good_fb)
            total_q = round(total)
            rows.append([p, vendor, round(q, 1), round(total_q * rej), total_q, fb])
    return rows


def dirty_rows():
    """~50 baris: 30 error + 20 valid. Nomor PO diawali PO-KTR agar mudah dikenali."""
    base = date(2026, 11, 10)
    valid = []
    for i in range(20):
        name, cat, com, bu, price, units, qrange, lam, vendors, karton = ITEMS[i % len(ITEMS) * 3 % len(ITEMS)]
        unit = units[0][0]
        size = unit_size(unit, karton)
        qty = max(1, round(random.uniform(*qrange) / size)) if unit != "gram" else 1000
        up = int(round(price * size / 10) * 10) if price * size >= 100 else int(price * size)
        d = base + timedelta(days=i % 5)
        valid.append([f"PO-KTR-{i + 1:03d}", d.isoformat(), V[vendors[0]], cat, name, com or "", qty, UNIT_DISPLAY.get(unit, unit), up, "", (d + timedelta(days=4)).isoformat()])

    g = "Gula Pasir Kristal Putih"
    errors = [
        # (po_no suffix, po_date, vendor, category, item, commodity, qty, unit, unit_price, delivered, promised)
        ["2026-13-05", V["manis"], "Bahan Baku", g, "Gula Pasir", "40", "sak 50 kg", "742000", "", "2026-11-14"],
        ["31/11/2026", V["manis"], "Bahan Baku", g, "Gula Pasir", "40", "sak 50 kg", "742000", "", "2026-11-14"],
        ["kemarin", V["manis"], "Bahan Baku", g, "Gula Pasir", "40", "sak 50 kg", "742000", "", "2026-11-14"],
        ["2026-02-30", V["gandum"], "Bahan Baku", "Tepung Terigu Protein Tinggi", "Tepung Terigu", "120", "sak 25 kg", "245000", "", "2026-03-03"],
        ["", V["gandum"], "Bahan Baku", "Tepung Terigu Protein Tinggi", "Tepung Terigu", "120", "sak 25 kg", "245000", "", "2026-11-14"],
        ["2026-11-11", V["manis"], "Bahan Baku", g, "Gula Pasir", "1.234,50", "kg", "14840", "", "2026-11-14"],
        ["2026-11-11", V["manis"], "Bahan Baku", g, "Gula Pasir", "40", "sak 50 kg", "742.000,00", "", "2026-11-14"],
        ["2026-11-11", V["sawit"], "Bahan Baku", "Minyak Goreng Sawit", "Minyak Nabati", "2.500", "liter", "17500", "", "2026-11-14"],
        ["2026-11-11", V["sawit"], "Bahan Baku", "Minyak Goreng Sawit", "Minyak Nabati", "50", "jerigen 18 L", "Rp 315.000", "", "2026-11-14"],
        ["2026-11-11", V["susu"], "Bahan Baku", "Susu Bubuk Full Cream", "Susu Bubuk", "abc", "sak 25 kg", "1450000", "", "2026-11-14"],
        ["2026-11-11", "", "Bahan Baku", g, "Gula Pasir", "40", "sak 50 kg", "742000", "", "2026-11-14"],
        ["2026-11-11", "   ", "Kemasan", "Box Kue Karton", "", "30", "karton", "250000", "", "2026-11-18"],
        ["2026-11-11", "", "Bahan Baku", "Telur Ayam Negeri", "Telur Ayam", "500", "kg", "28000", "", "2026-11-13"],
        ["2026-11-11", V["manis"], "Bahan Baku", g, "Gula Pasir", "4", "drum", "742000", "", "2026-11-14"],
        ["2026-11-11", V["sawit"], "Bahan Baku", "Minyak Goreng Sawit", "Minyak Nabati", "10", "bal", "17500", "", "2026-11-14"],
        ["2026-11-11", V["karton"], "Kemasan", "Lakban Bening", "", "5", "lusin", "12000", "", "2026-11-18"],
        ["2026-11-11", V["kakao"], "Bahan Baku", "Cokelat Bubuk", "Cokelat Bubuk", "100", "ton", "85000", "", "2026-11-14"],
        ["2026-11-11", V["teknik"], "Sparepart", "Bearing 6205", "", "4", "kg", "85000", "", "2026-11-20"],  # satuan tidak cocok dengan item (pcs)
        ["2026-11-11", V["manis"], "Bahan Baku", g, "Gula Pasir", "-10", "sak 50 kg", "742000", "", "2026-11-14"],
        ["2026-11-11", V["manis"], "Bahan Baku", g, "Gula Pasir", "0", "sak 50 kg", "742000", "", "2026-11-14"],
        ["2026-11-11", V["manis"], "Bahan Baku", g, "Gula Pasir", "40", "sak 50 kg", "-5000", "", "2026-11-14"],
        ["2026-11-11", V["manis"], "Bahan Baku", g, "Gula Pasir", "", "sak 50 kg", "742000", "", "2026-11-14"],
        ["2026-11-11", V["manis"], "Bahan Bakar", g, "Gula Pasir", "40", "sak 50 kg", "742000", "", "2026-11-14"],
        ["2026-11-11", V["manis"], "", g, "Gula Pasir", "40", "sak 50 kg", "742000", "", "2026-11-14"],
        ["2026-11-11", V["manis"], "Bahan Baku", "", "Gula Pasir", "40", "sak 50 kg", "742000", "", "2026-11-14"],
        ["2026-11-11", V["manis"], "Bahan Baku", g, "Gula Batu Merah", "40", "sak 50 kg", "742000", "", "2026-11-14"],  # komoditas tidak dikenal
        ["2026-11-11", V["manis"], "Bahan Baku", g, "Gula Pasir", "40", "sak 50 kg", "742000", "2026-11-31", "2026-11-14"],
        ["2026-11-11", V["manis"], "Bahan Baku", g, "Gula Pasir", "40", "sak 50 kg", "742000", "", "14-11-2026"],
        ["2026-11-11", V["manis"], "Bahan Baku", g, "Gula Pasir", "40", "sak 50 kg", "742000", "", ""],
        ["2026-11-11", V["manis"], "Bahan Baku", g, "Gula Pasir", "40", "sak 50 kg", "742000", "2026-11-05", "2026-11-14"],  # delivered sebelum po_date
    ]
    out = []
    for i, e in enumerate(errors):
        out.append([f"PO-KTR-E{i + 1:02d}"] + e)
    rows = []
    # selang-seling valid & error supaya preview realistis
    vi, ei = 0, 0
    while vi < len(valid) or ei < len(out):
        if ei < len(out):
            rows.append(out[ei]); ei += 1
        if ei < len(out) and ei % 3 == 0 and vi < len(valid):
            rows.append(valid[vi]); vi += 1
        if ei >= len(out) and vi < len(valid):
            rows.append(valid[vi]); vi += 1
    return rows, len(out), len(valid)


def split_demo_rows():
    """Demo deteksi split PO: 4 PO kecil (masing-masing < Rp 100 jt) ke vendor yang sama di hari
    yang sama, total Rp 384 jt. Tidak memakai random, jadi tidak mengubah file lain."""
    d, sent, promised = "2026-11-13", "2026-11-14", "2026-11-16"
    v, cat = "PT Karton Indah Perkasa", "Kemasan"
    return [
        ["PO-DEMO-SPLIT-01", d, v, cat, "Box Kue Karton", "", 395, "karton", 250000, sent, promised],
        ["PO-DEMO-SPLIT-02", d, v, cat, "Box Kue Karton", "", 390, "karton", 250000, sent, promised],
        ["PO-DEMO-SPLIT-03", d, v, cat, "Box Kue Karton", "", 380, "karton", 250000, sent, promised],
        ["PO-DEMO-SPLIT-04", d, v, cat, "Box Kue Karton", "", 300, "karton", 250000, sent, promised],
        ["PO-DEMO-SPLIT-04", d, v, cat, "Mika Kue", "", 40, "karton", 450000, sent, promised],
    ]


def main():
    lines = gen_lines()
    write_po(lines, os.path.join(HERE, "po_12bulan.csv"))

    ratings = gen_ratings(lines)
    with open(os.path.join(HERE, "penilaian_vendor_12bulan.csv"), "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["period", "vendor", "quality_score", "reject_qty", "total_qty", "feedback"])
        w.writerows(ratings)

    rows, n_err, n_ok = dirty_rows()
    with open(os.path.join(HERE, "po_kotor.csv"), "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["po_no", "po_date", "vendor", "category", "item", "commodity", "qty", "unit", "unit_price", "delivered_date", "promised_date"])
        w.writerows(rows)

    with open(os.path.join(HERE, "po_split_demo.csv"), "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["po_no", "po_date", "vendor", "category", "item", "commodity", "qty", "unit", "unit_price", "delivered_date", "promised_date"])
        w.writerows(split_demo_rows())

    master = {
        "categories": CATEGORIES,
        "commodities": [{"name": n, "base_unit": u} for n, u in COMMODITIES],
        "global_units": [{"from_unit": a, "base_unit": b, "to_base": c} for a, b, c in GLOBAL_UNITS],
        "items": [
            {"name": it[0], "category": it[1], "commodity": it[2], "base_unit": it[3],
             "karton_to_base": it[9]} for it in ITEMS
        ],
    }
    with open(os.path.join(HERE, "master.json"), "w", encoding="utf-8") as f:
        json.dump(master, f, ensure_ascii=False, indent=2)

    # total spend per bulan untuk cek manual
    tot = defaultdict(int)
    cnt = defaultdict(set)
    for r in lines:
        m = r["po_date"].strftime("%Y-%m")
        tot[m] += r["qty"] * r["unit_price"]
        cnt[m].add(r["po_no"])
    with open(os.path.join(HERE, "expected_totals.csv"), "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["month", "total_spend", "po_count"])
        for m in sorted(tot):
            w.writerow([m, tot[m], len(cnt[m])])

    print(f"PO lines: {len(lines)}, PO: {len(set(r['po_no'] for r in lines))}, ratings: {len(ratings)}, dirty: {n_err} error + {n_ok} valid")


if __name__ == "__main__":
    main()
