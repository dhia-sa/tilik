"""Buat snapshot JSON untuk versi demo hanya-baca (GitHub Pages).

Jalankan saat server Tilik lokal hidup (http://127.0.0.1:8080):

    python scripts/snapshot_demo.py frontend/dist-demo/demo

Script login sebagai Data Admin demo, lalu menyimpan respons setiap GET yang dipakai frontend
untuk 5 preset periode (tanpa filter kategori/vendor), plus jawaban contoh pertanyaan Tanya Data.
Nama file dibuat dengan aturan yang sama seperti demoKey() di frontend/src/demo.ts.
"""

import http.cookiejar
import json
import re
import sys
import urllib.parse
import urllib.request
from datetime import date, timedelta
from pathlib import Path

BASE = "http://127.0.0.1:8080/api"
ROOT = Path(__file__).resolve().parent.parent


def demo_key(path: str) -> str:
    parts = path.split("?")
    p, q = parts[0], (parts[1] if len(parts) > 1 else "")
    entries = [(k, v) for k, v in urllib.parse.parse_qsl(q, keep_blank_values=True) if v != ""]
    entries.sort(key=lambda kv: kv[0])
    full = p + ("?" + "&".join(f"{k}={v}" for k, v in entries) if entries else "")
    return re.sub(r"[^A-Za-z0-9]+", "_", full).strip("_").lower()


def add_months(d: str, n: int) -> str:
    y, m = int(d[:4]), int(d[5:7])
    t = (y * 12 + (m - 1)) + n
    return f"{t // 12:04d}-{t % 12 + 1:02d}-01"


class Client:
    def __init__(self) -> None:
        self.op = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(http.cookiejar.CookieJar()))

    def req(self, method: str, path: str, body: dict | None = None):
        data = json.dumps(body).encode() if body is not None else None
        r = urllib.request.Request(BASE + urllib.parse.quote(path, safe="/?=&-_.:"), data=data, method=method)
        if data is not None:
            r.add_header("Content-Type", "application/json")
        with self.op.open(r, timeout=300) as res:
            return json.loads(res.read().decode("utf-8") or "null")


def main() -> None:
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "frontend/dist-demo/demo")
    out.mkdir(parents=True, exist_ok=True)
    c = Client()
    c.req("POST", "/auth/login", {"email": "dataadmin@tilik.local", "password": "dataadmin123"})
    saved = 0

    def save(path: str, data=None) -> None:
        nonlocal saved
        if data is None:
            data = c.req("GET", path)
        (out / f"{demo_key(path)}.json").write_text(json.dumps(data, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
        saved += 1

    meta = c.req("GET", "/meta")
    save("/meta", meta)
    ref = meta["reference_date"]
    ms = ref[:8] + "01"
    presets = [(ms, ref), (add_months(ms, -2), ref), (add_months(ms, -5), ref), (add_months(ms, -11), ref), (ref[:4] + "-01-01", ref)]
    presets.append((meta["default_from"], meta["default_to"]))
    for f, t in dict.fromkeys(presets):
        qs = f"?from={f}&to={t}"
        for p in ["/overview", "/spend/by-category", "/vendors/scorecard", "/vendors/split-po"]:
            save(p + qs)
        for cat in meta["categories"]:
            save(f"/spend/category/{cat['id']}{qs}")
        for v in meta["vendors"]:
            save(f"/vendors/{v['id']}{qs}")
            save(f"/vendors/split-po{qs}&vendor={v['id']}")
        print("periode", f, "-", t, "selesai", flush=True)

    save("/commodities")
    for cm in meta["commodities"]:
        save(f"/commodities/{cm['id']}")
    since = (date.fromisoformat(ref) - timedelta(days=13)).isoformat()
    save(f"/alerts?from={since}&limit=8")
    save("/imports")
    save("/settings")

    ask_src = (ROOT / "frontend/src/pages/Ask.tsx").read_text(encoding="utf-8")
    block = re.search(r"const EXAMPLES = \[(.*?)\]", ask_src, re.S)
    examples = re.findall(r"'([^']+)'", block.group(1)) if block else []
    for q in examples:
        save(f"/ask?q={q.strip().lower()}", c.req("POST", "/ask", {"question": q}))
        print("tanya:", q, flush=True)
    save("/ask/history")
    print(f"{saved} snapshot disimpan di {out}")


if __name__ == "__main__":
    main()
