"""Barrido de variantes de estrategia a 1x, con validación en dos mitades de los datos."""
import json, pathlib, itertools
import backtest as bt

DATA = bt.DATA
coins = ["BTC", "ETH", "SOL"]

# Partir datos en dos mitades para detectar sobreajuste
for c in coins:
    rows = json.loads((DATA / f"{c}.json").read_text())
    h = len(rows) // 2
    (DATA / f"{c}_A.json").write_text(json.dumps(rows[:h]))
    (DATA / f"{c}_B.json").write_text(json.dumps(rows[h:]))
for k in list(bt.MAX_LEV):
    bt.MAX_LEV[k + "_A"] = bt.MAX_LEV[k]
    bt.MAX_LEV[k + "_B"] = bt.MAX_LEV[k]

grid = {
    "vol_mult": [1.25, 2.0, 3.0, 5.0],
    "stop": [0.005, 0.01, 0.02, 0.0],
    "trend_filter": [0.0, 1.0, 2.0],
    "max_hold": [None, 30],
}
res = []
for vm, st, tf, mh in itertools.product(*grid.values()):
    kw = dict(vol_mult=vm, stop=st, trend_filter=tf, max_hold=mh)
    a = [bt.run(c + "_A", 19, 1, **kw)["ret"] for c in coins]
    b = [bt.run(c + "_B", 19, 1, **kw)["ret"] for c in coins]
    res.append((sum(a) / 3, sum(b) / 3, min(a + b), kw))

res.sort(key=lambda r: r[2], reverse=True)
print("Top 10 por PEOR resultado (mitad A/B, 3 monedas), apalancamiento 1x, capital $19")
print(f"{'ret A %':>8}{'ret B %':>8}{'peor %':>8}  params")
for a, b, w, kw in res[:10]:
    print(f"{100*a:>8.2f}{100*b:>8.2f}{100*w:>8.2f}  {kw}")
pos = [r for r in res if r[0] > 0 and r[1] > 0]
print(f"\nVariantes positivas en AMBAS mitades: {len(pos)} de {len(res)}")
for a, b, w, kw in pos[:10]:
    print(f"{100*a:>8.2f}{100*b:>8.2f}{100*w:>8.2f}  {kw}")
