"""Descarga datos públicos de Hyperliquid para investigación:
- velas de 1h (últimas ~5000 = ~208 días)
- historial de funding horario
- contexto actual de todos los perps (funding, volumen, OI)
"""
import json, sys, time, urllib.request, pathlib

OUT = pathlib.Path(__file__).parent / "data"
OUT.mkdir(exist_ok=True)
URL = "https://api.hyperliquid.xyz/info"


def info(payload):
    req = urllib.request.Request(URL, data=json.dumps(payload).encode(),
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.load(r)


def candles_1h(coin, days=210):
    end = int(time.time() * 1000)
    t = end - days * 86400_000
    rows = {}
    while t < end:
        chunk = info({"type": "candleSnapshot",
                      "req": {"coin": coin, "interval": "1h", "startTime": t,
                              "endTime": min(t + 4000 * 3600_000, end)}})
        if not chunk:
            t += 4000 * 3600_000
            continue
        for c in chunk:
            rows[c["t"]] = (c["t"], float(c["o"]), float(c["h"]), float(c["l"]), float(c["c"]), float(c["v"]))
        t = chunk[-1]["t"] + 3600_000
        time.sleep(1.0)
    return [rows[k] for k in sorted(rows)]


def funding(coin, days=210):
    end = int(time.time() * 1000)
    t = end - days * 86400_000
    out = {}
    while t < end:
        chunk = info({"type": "fundingHistory", "coin": coin, "startTime": t, "endTime": end})
        if not chunk:
            break
        for f in chunk:
            out[f["time"]] = float(f["fundingRate"])
        nt = chunk[-1]["time"] + 1
        if nt <= t:
            break
        t = nt
        time.sleep(1.0)
    return sorted(out.items())


if __name__ == "__main__":
    meta, ctxs = info({"type": "metaAndAssetCtxs"})
    universe = []
    for m, c in zip(meta["universe"], ctxs):
        if m.get("isDelisted"):
            continue
        universe.append({"coin": m["name"], "maxLev": m["maxLeverage"], "szDecimals": m["szDecimals"],
                         "funding": float(c["funding"]), "dayNtlVlm": float(c["dayNtlVlm"]),
                         "oi": float(c["openInterest"]) * float(c["markPx"] or 0)})
    universe.sort(key=lambda x: -x["dayNtlVlm"])
    (OUT / "universe.json").write_text(json.dumps(universe))
    coins = sys.argv[1:] or [u["coin"] for u in universe[:15]]
    print("Monedas:", coins)
    for c in coins:
        if (OUT / f"{c}_funding.json").exists() and (OUT / f"{c}_1h.json").exists():
            continue
        (OUT / f"{c}_1h.json").write_text(json.dumps(candles_1h(c)))
        (OUT / f"{c}_funding.json").write_text(json.dumps(funding(c)))
        print(c, "ok")
