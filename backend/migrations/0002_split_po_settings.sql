-- Pengaturan deteksi kemungkinan split PO (bonus).
INSERT OR IGNORE INTO settings(key, value) VALUES
    ('split_po_approval_limit', '100000000'),
    ('split_po_min_count', '3');
