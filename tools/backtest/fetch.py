"""Descarga velas de 1m de Hyperliquid (API pública) y las guarda en JSON."""
import json, sys, time, urllib.request, pathlib

OUT = pathlib.Path(__file__).parent / "data"
OUT.mkdir(exist_ok=True)
DAYS = 14

def candles(coin, start, end):
    body = json.dumps({"type": "candleSnapshot",
                       "req": {"coin": coin, "interval": "1m", "startTime": start, "endTime": end}}).encode()
    req = urllib.request.Request("https://api.hyperliquid.xyz/info", data=body,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.load(r)

for coin in sys.argv[1:]:
    end = int(time.time() * 1000)
    start = end - DAYS * 86400_000
    rows, t = {}, start
    while t < end:
        chunk = candles(coin, t, min(t + 4000 * 60_000, end))
        if not chunk:
            t += 4000 * 60_000
            continue
        for c in chunk:
            rows[c["t"]] = (c["t"], float(c["o"]), float(c["h"]), float(c["l"]), float(c["c"]), float(c["v"]))
        t = chunk[-1]["t"] + 60_000
        time.sleep(0.3)
    data = [rows[k] for k in sorted(rows)]
    (OUT / f"{coin}.json").write_text(json.dumps(data))
    print(coin, len(data), "velas")
