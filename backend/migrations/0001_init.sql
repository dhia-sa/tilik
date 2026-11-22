-- Tilik schema awal

CREATE TABLE users (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    email         TEXT NOT NULL UNIQUE,
    name          TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role          TEXT NOT NULL CHECK (role IN ('viewer', 'data_admin', 'admin')),
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE sessions (
    token      TEXT PRIMARY KEY,
    user_id    INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TEXT NOT NULL
);

CREATE TABLE import_batches (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    kind        TEXT NOT NULL CHECK (kind IN ('po', 'vendor_rating')),
    file_name   TEXT NOT NULL,
    rows_ok     INTEGER NOT NULL,
    rows_error  INTEGER NOT NULL,
    user_id     INTEGER REFERENCES users(id),
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    rolled_back INTEGER NOT NULL DEFAULT 0,
    rolled_back_at TEXT,
    duration_ms INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE vendors (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    name             TEXT NOT NULL UNIQUE,
    created_batch_id INTEGER REFERENCES import_batches(id)
);

CREATE TABLE categories (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

CREATE TABLE commodities (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    name      TEXT NOT NULL UNIQUE,
    base_unit TEXT NOT NULL
);

CREATE TABLE items (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    name             TEXT NOT NULL UNIQUE,
    category_id      INTEGER NOT NULL REFERENCES categories(id),
    commodity_id     INTEGER REFERENCES commodities(id),
    base_unit        TEXT NOT NULL,
    created_batch_id INTEGER REFERENCES import_batches(id)
);

-- item_id NULL = konversi global. base_unit dipakai untuk memastikan dimensi cocok (kg/liter/pcs).
CREATE TABLE unit_conversions (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id   INTEGER REFERENCES items(id) ON DELETE CASCADE,
    from_unit TEXT NOT NULL,
    base_unit TEXT NOT NULL,
    to_base   REAL NOT NULL CHECK (to_base > 0)
);
CREATE UNIQUE INDEX ux_unit_conv ON unit_conversions(COALESCE(item_id, 0), from_unit);

CREATE TABLE purchase_orders (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    po_no           TEXT NOT NULL,
    po_date         TEXT NOT NULL,           -- YYYY-MM-DD
    vendor_id       INTEGER NOT NULL REFERENCES vendors(id),
    item_id         INTEGER NOT NULL REFERENCES items(id),
    qty             REAL NOT NULL,
    unit            TEXT NOT NULL,
    unit_price      INTEGER NOT NULL,        -- Rupiah per satuan asli
    amount          REAL NOT NULL,           -- qty * unit_price
    base_qty        REAL NOT NULL,
    base_unit_price REAL NOT NULL,           -- Rupiah per satuan dasar
    promised_date   TEXT NOT NULL,
    delivered_date  TEXT,
    iso_week        TEXT NOT NULL,           -- YYYY-Www (Senin-Minggu)
    batch_id        INTEGER NOT NULL REFERENCES import_batches(id)
);
CREATE UNIQUE INDEX ux_po_line ON purchase_orders(po_no, item_id);
CREATE INDEX ix_po_date ON purchase_orders(po_date);
CREATE INDEX ix_po_vendor ON purchase_orders(vendor_id, po_date);
CREATE INDEX ix_po_item ON purchase_orders(item_id, po_date);
CREATE INDEX ix_po_batch ON purchase_orders(batch_id);

CREATE TABLE vendor_ratings (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    period        TEXT NOT NULL,             -- YYYY-MM
    vendor_id     INTEGER NOT NULL REFERENCES vendors(id),
    quality_score REAL NOT NULL,
    reject_qty    REAL NOT NULL,
    total_qty     REAL NOT NULL,
    feedback      TEXT,
    batch_id      INTEGER NOT NULL REFERENCES import_batches(id)
);
CREATE UNIQUE INDEX ux_rating ON vendor_ratings(period, vendor_id);
CREATE INDEX ix_rating_batch ON vendor_ratings(batch_id);

-- Agregat mingguan harga komoditas (dihitung ulang setiap import/rollback)
CREATE TABLE commodity_weekly (
    commodity_id INTEGER NOT NULL REFERENCES commodities(id),
    week         TEXT NOT NULL,
    week_start   TEXT NOT NULL,
    base_qty     REAL NOT NULL,
    spend        REAL NOT NULL,
    avg_price    REAL NOT NULL,              -- rata-rata tertimbang qty (Rp / satuan dasar)
    po_lines     INTEGER NOT NULL,
    PRIMARY KEY (commodity_id, week)
);

CREATE TABLE alerts (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    kind         TEXT NOT NULL CHECK (kind IN ('price', 'vendor')),
    ref_id       INTEGER NOT NULL,           -- commodity_id atau vendor_id
    week         TEXT NOT NULL,              -- YYYY-Www (harga) atau YYYY-MM (vendor)
    period_start TEXT NOT NULL,
    message      TEXT NOT NULL,
    value        REAL NOT NULL,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX ix_alerts_period ON alerts(period_start);

CREATE TABLE ask_history (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id         INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    question        TEXT NOT NULL,
    tool_calls_json TEXT NOT NULL,
    answer          TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE ai_calls (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    provider    TEXT NOT NULL,
    purpose     TEXT NOT NULL,
    prompt      TEXT NOT NULL,
    raw_output  TEXT,
    ok          INTEGER NOT NULL,
    error       TEXT,
    duration_ms INTEGER NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT INTO settings(key, value) VALUES
    ('price_alert_threshold_pct', '5'),
    ('price_alert_sigma', '2'),
    ('price_alert_min_dev_pct', '2'),
    ('vendor_alert_on_time_pct', '70'),
    ('vendor_alert_reject_pct', '5'),
    ('weight_on_time', '40'),
    ('weight_quality', '35'),
    ('weight_price', '25');
