"""¿Se puede ganar en las caídas? Compara, en velas de 1h, con costos reales:

  A. Tendencia larga actual (ruptura de máximo 168h, stop/trailing 3 ATR, salida mínimo 24h)
  B. Tendencia corta (ruptura de MÍNIMO 168h, espejo de A; los cortos cobran funding positivo)
  C. Retroceso en tendencia alcista ("buy the dip"): precio sobre la EMA de 200h y
     caída de k ATR bajo la EMA de 20h -> compra; sale al volver a la EMA20, por stop
     o por tiempo.
  D. Rebote en sobreventa sin filtro de tendencia (RSI14 < 25), salida RSI > 50 / tiempo.

Cada operación se mide en R (ganancia / riesgo inicial) neta de costos y funding.
Con 1% de riesgo por operación, 1R ≈ 1% de la cuenta. Validación en dos mitades.
"""
import json, pathlib, statistics as st
from trend import load, DATA

TAKER, SLIP = 0.00045, 0.0005
COST = 2 * (TAKER + SLIP)  # ida y vuelta, fracción del nocional


def indicators(rows, atr_n=14):
    atr, prev = None, None
    e20 = e200 = None
    gains = losses = None
    out = []
    for t, o, h, l, c, v in rows:
        tr = h - l if prev is None else max(h - l, abs(h - prev), abs(l - prev))
        atr = tr if atr is None else atr + (tr - atr) / atr_n
        e20 = c if e20 is None else e20 + (c - e20) * 2 / 21
        e200 = c if e200 is None else e200 + (c - e200) * 2 / 201
        if prev is not None:
            ch = c - prev
            g, ls = max(ch, 0), max(-ch, 0)
            gains = g if gains is None else gains + (g - gains) / 14
            losses = ls if losses is None else losses + (ls - losses) / 14
        rsi = 100 - 100 / (1 + gains / losses) if losses else 50
        prev = c
        out.append(dict(t=t, o=o, h=h, l=l, c=c, atr=atr, e20=e20, e200=e200, rsi=rsi))
    return out


def simulate(rows, fh, strategy, start, end, **p):
    ind = indicators(rows)
    trades = []
    pos = None  # dict(side, entry, stop, risk, opened, t_entry)
    N = 168
    for i in range(max(N, 200) + 1, end):
        b = ind[i]
        if pos and i >= start:
            side = pos["side"]
            # funding: el largo paga funding positivo, el corto lo cobra
            pos["fund"] += side * -fh.get((b["t"] // 3600_000) * 3600_000, 0.0)
            hit = (b["l"] <= pos["stop"]) if side > 0 else (b["h"] >= pos["stop"])
            exit_px = None
            if hit:
                exit_px = min(b["o"], pos["stop"]) if side > 0 else max(b["o"], pos["stop"])
            elif strategy in ("long", "short"):
                hh = max(x["h"] for x in ind[i - 24:i]); ll = min(x["l"] for x in ind[i - 24:i])
                trail = b["c"] - side * p["stop_atr"] * b["atr"]
                pos["stop"] = max(pos["stop"], trail) if side > 0 else min(pos["stop"], trail)
                if (side > 0 and b["c"] < ll) or (side < 0 and b["c"] > hh):
                    exit_px = b["c"]
            elif strategy == "dip":
                if b["c"] >= b["e20"] or i - pos["opened"] >= p["max_hold"]:
                    exit_px = b["c"]
            elif strategy == "rsi":
                if b["rsi"] > 50 or i - pos["opened"] >= p["max_hold"]:
                    exit_px = b["c"]
            if exit_px is not None:
                move = side * (exit_px - pos["entry"]) / pos["entry"]
                r = (move - COST + pos["fund"]) / pos["risk"]
                trades.append((b["t"], r))
                pos = None
            continue
        if pos or i < start:
            continue

        sig = None
        if strategy == "long":
            if b["c"] > max(x["h"] for x in ind[i - N:i]):
                sig = 1
        elif strategy == "short":
            if b["c"] < min(x["l"] for x in ind[i - N:i]):
                sig = -1
        elif strategy == "dip":
            if b["c"] > b["e200"] and b["c"] < b["e20"] - p["k"] * b["atr"]:
                sig = 1
        elif strategy == "rsi":
            if b["rsi"] < p["rsi_in"]:
                sig = 1
        if sig:
            dist = p["stop_atr"] * b["atr"]
            pos = dict(side=sig, entry=b["c"], stop=b["c"] - sig * dist,
                       risk=dist / b["c"], opened=i, fund=0.0)
    return trades


def summary(trades):
    if not trades:
        return dict(n=0, win=0, avg=0, tot=0)
    rs = [r for _, r in trades]
    return dict(n=len(rs), win=sum(r > 0 for r in rs) / len(rs), avg=st.mean(rs), tot=sum(rs))


CONFIGS = [
    ("A. Tendencia larga (actual)", "long", dict(stop_atr=3.0)),
    ("B. Tendencia corta", "short", dict(stop_atr=3.0)),
    ("C. Retroceso 1.5 ATR, 24h", "dip", dict(k=1.5, stop_atr=2.0, max_hold=24)),
    ("C. Retroceso 2 ATR, 24h", "dip", dict(k=2.0, stop_atr=2.0, max_hold=24)),
    ("C. Retroceso 2 ATR, 48h", "dip", dict(k=2.0, stop_atr=3.0, max_hold=48)),
    ("C. Retroceso 3 ATR, 48h", "dip", dict(k=3.0, stop_atr=3.0, max_hold=48)),
    ("D. Sobreventa RSI<25, 24h", "rsi", dict(rsi_in=25, stop_atr=2.0, max_hold=24)),
    ("D. Sobreventa RSI<20, 48h", "rsi", dict(rsi_in=20, stop_atr=3.0, max_hold=48)),
]

if __name__ == "__main__":
    coins = [f.name.replace("_1h.json", "") for f in sorted(DATA.glob("*_1h.json"))]
    data = {c: load(c) for c in coins}
    print(f"{len(coins)} monedas, velas 1h, ~{len(next(iter(data.values()))[0]) // 24} días; resultados en R netos de costos")
    print(f"{'estrategia':30}{'mitad':>6}{'ops':>6}{'acierto':>9}{'R medio':>9}{'R total':>9}")
    for name, strat, p in CONFIGS:
        for label, frac in (("1a", (0, .5)), ("2a", (.5, 1))):
            allt = []
            for c, (rows, fh) in data.items():
                n = len(rows)
                allt += simulate(rows, fh, strat, int(n * frac[0]), int(n * frac[1]), **p)
            s = summary(allt)
            print(f"{name:30}{label:>6}{s['n']:>6}{100 * s['win']:>8.0f}%{s['avg']:>+9.3f}{s['tot']:>+9.1f}")
