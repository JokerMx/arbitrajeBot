"""Simulación de CARTERA: una sola cuenta de $50 operando Donchian en varias monedas
a la vez, con equity compartido, límite de apalancamiento total y mínimo de $10.
Compara contra buy & hold para descartar que el resultado sea solo "mercado alcista".
"""
import json, pathlib, sys
from trend import load, DATA, TAKER, SLIP

CAPITAL = 50.0


def run(coins, *, slow=168, fast=24, stop_atr=3.0, atr_n=14, risk=0.01, max_total_lev=3.0,
        max_positions=4, start_frac=0.0, end_frac=1.0, regime_ema=0, verbose=False):
    data = {c: load(c) for c in coins}
    # alinear por timestamp
    times = sorted(set.intersection(*[set(r[0] for r in data[c][0]) for c in coins]))
    idx = {c: {r[0]: r for r in data[c][0]} for c in coins}
    n = len(times)
    t0, t1 = int(n * start_frac), int(n * end_frac)

    eq = CAPITAL
    pos = {}  # coin -> dict(size, entry, stop)
    st = {c: dict(atr=None, prev=None, hh=[], ll=[], ema=None) for c in coins}
    peak, mdd, closed = CAPITAL, 0.0, []
    for k, t in enumerate(times[:t1]):
        active = k >= t0
        for c in coins:
            _, o, h, l, cl, v = idx[c][t]
            s = st[c]
            p = pos.get(c)
            if p and active:
                eq -= p["size"] * o * data[c][1].get(t, 0.0)
                if l <= p["stop"]:
                    px = min(o, p["stop"]) * (1 - SLIP)
                    pnl = p["size"] * (px - p["entry"]) - p["size"] * px * TAKER
                    eq += pnl; closed.append(pnl); del pos[c]; p = None
            tr = h - l if s["prev"] is None else max(h - l, abs(h - s["prev"]), abs(l - s["prev"]))
            s["atr"] = tr if s["atr"] is None else s["atr"] + (tr - s["atr"]) / atr_n
            s["prev"] = cl
            if regime_ema:
                a = 2 / (regime_ema + 1)
                s["ema"] = cl if s["ema"] is None else s["ema"] + a * (cl - s["ema"])
            s["hh"].append(h); s["ll"].append(l)
            if len(s["hh"]) > slow + 1:
                s["hh"].pop(0); s["ll"].pop(0)
            if len(s["hh"]) <= slow or not active:
                continue
            if p:
                p["stop"] = max(p["stop"], cl - stop_atr * s["atr"])
                if cl < min(s["ll"][-fast - 1:-1]):
                    px = cl * (1 - SLIP)
                    pnl = p["size"] * (px - p["entry"]) - p["size"] * px * TAKER
                    eq += pnl; closed.append(pnl); del pos[c]
                continue
            breakout = cl > max(s["hh"][:-1])
            if regime_ema and cl < s["ema"]:
                breakout = False
            if breakout and len(pos) < max_positions and eq > 10:
                dist = stop_atr * s["atr"]
                notional = eq * risk / (dist / cl)
                used = sum(q["size"] * idx[q_c][t][4] for q_c, q in pos.items())
                notional = min(notional, eq * max_total_lev - used)
                if notional >= 10.0:
                    px = cl * (1 + SLIP)
                    eq -= notional * TAKER
                    pos[c] = dict(size=notional / px, entry=px, stop=px - dist)
        if active:
            mtm = eq + sum(q["size"] * (idx[c][t][4] - q["entry"]) for c, q in pos.items())
            peak = max(peak, mtm); mdd = max(mdd, (peak - mtm) / peak)
    final = eq + sum(q["size"] * (idx[c][times[t1 - 1]][4] - q["entry"]) for c, q in pos.items())
    days = (times[t1 - 1] - times[t0]) / 86400_000
    bh = sum(idx[c][times[t1 - 1]][4] / idx[c][times[t0]][4] - 1 for c in coins) / len(coins)
    wins = sum(x > 0 for x in closed)
    return dict(ret=final / CAPITAL - 1, mdd=mdd, trades=len(closed),
                win=wins / len(closed) if closed else 0, days=days, bh=bh,
                avg_win=sum(x for x in closed if x > 0) / max(wins, 1),
                avg_loss=sum(x for x in closed if x <= 0) / max(len(closed) - wins, 1))


def line(name, r):
    print(f"{name:34}{100 * r['ret']:>8.1f}%{100 * r['mdd']:>8.1f}%{r['trades']:>6}{100 * r['win']:>6.0f}%"
          f"{r['avg_win']:>8.2f}{r['avg_loss']:>8.2f}{100 * r['bh']:>9.1f}%{r['days']:>6.0f}")


if __name__ == "__main__":
    coins = [f.name.replace("_1h.json", "") for f in sorted(DATA.glob("*_1h.json"))]
    print(f"{'escenario':34}{'ret':>9}{'maxDD':>9}{'ops':>6}{'win':>7}{'$gan':>8}{'$perd':>8}{'B&H prom':>10}{'días':>6}")
    for frac in [(0, .5), (.5, 1), (0, 1)]:
        tag = {(0, .5): "1a mitad", (.5, 1): "2a mitad", (0, 1): "completo"}[frac]
        for risk in (0.005, 0.01, 0.02):
            line(f"{tag} riesgo {risk:.1%}", run(coins, risk=risk, start_frac=frac[0], end_frac=frac[1]))
        line(f"{tag} riesgo 1% + filtro EMA200", run(coins, risk=0.01, regime_ema=200,
                                                      start_frac=frac[0], end_frac=frac[1]))
    print("\nSolo 4 monedas líquidas (BTC ETH SOL HYPE), riesgo 1%:")
    for frac in [(0, .5), (.5, 1), (0, 1)]:
        line(f"  {frac}", run(["BTC", "ETH", "SOL", "HYPE"], risk=0.01, start_frac=frac[0], end_frac=frac[1]))
