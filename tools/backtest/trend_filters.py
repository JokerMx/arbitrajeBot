"""Filtros sobre la estrategia de tendencia actual (cartera de $500, riesgo 0,5%):
  - btc:     solo entrar si BTC está sobre su media móvil de 7 días (168 h)
  - funding: no entrar si el funding medio de las últimas 24 h de la moneda supera un
             umbral (mercado sobrecomprado, muchos largos apalancados)
Misma lógica que portfolio.py; validación en dos mitades.
"""
from trend import load, DATA, TAKER, SLIP

CAPITAL = 500.0
BASE_FUNDING = 0.0000125  # 0,00125%/h ≈ 11% anual: el funding "normal" de Hyperliquid


def run(coins, *, btc_filter=False, funding_max=None, slow=168, fast=24, stop_atr=3.0, atr_n=14,
        risk=0.005, max_total_lev=3.0, max_positions=4, start_frac=0.0, end_frac=1.0):
    data = {c: load(c) for c in coins}
    times = sorted(set.intersection(*[set(r[0] for r in data[c][0]) for c in coins]))
    idx = {c: {r[0]: r for r in data[c][0]} for c in coins}
    n = len(times)
    t0, t1 = int(n * start_frac), int(n * end_frac)
    eq, pos = CAPITAL, {}
    st = {c: dict(atr=None, prev=None, hh=[], ll=[], fund=[]) for c in coins}
    btc_ema = None
    peak, mdd, closed, skipped = CAPITAL, 0.0, [], 0
    for k, t in enumerate(times[:t1]):
        active = k >= t0
        b = idx["BTC"][t][4]
        btc_ema = b if btc_ema is None else btc_ema + (b - btc_ema) * 2 / 169
        for c in coins:
            _, o, h, l, cl, v = idx[c][t]
            s = st[c]
            f = data[c][1].get(t, 0.0)
            s["fund"].append(f)
            if len(s["fund"]) > 24:
                s["fund"].pop(0)
            p = pos.get(c)
            if p and active:
                eq -= p["size"] * o * f
                if l <= p["stop"]:
                    px = min(o, p["stop"]) * (1 - SLIP)
                    pnl = p["size"] * (px - p["entry"]) - p["size"] * px * TAKER
                    eq += pnl; closed.append(pnl); del pos[c]; p = None
            tr = h - l if s["prev"] is None else max(h - l, abs(h - s["prev"]), abs(l - s["prev"]))
            s["atr"] = tr if s["atr"] is None else s["atr"] + (tr - s["atr"]) / atr_n
            s["prev"] = cl
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
            if not cl > max(s["hh"][:-1]):
                continue
            # --- filtros ---
            if btc_filter and b < btc_ema:
                skipped += 1
                continue
            if funding_max is not None and sum(s["fund"]) / len(s["fund"]) > funding_max:
                skipped += 1
                continue
            if len(pos) < max_positions and eq > 10:
                dist = stop_atr * s["atr"]
                notional = eq * risk / (dist / cl)
                used = sum(q["size"] * idx[qc][t][4] for qc, q in pos.items())
                notional = min(notional, eq * max_total_lev - used)
                if notional >= 10.0:
                    px = cl * (1 + SLIP)
                    eq -= notional * TAKER
                    pos[c] = dict(size=notional / px, entry=px, stop=px - dist)
        if active:
            mtm = eq + sum(q["size"] * (idx[c][t][4] - q["entry"]) for c, q in pos.items())
            peak = max(peak, mtm); mdd = max(mdd, (peak - mtm) / peak)
    final = eq + sum(q["size"] * (idx[c][times[t1 - 1]][4] - q["entry"]) for c, q in pos.items())
    wins = sum(x > 0 for x in closed)
    return dict(ret=final / CAPITAL - 1, mdd=mdd, trades=len(closed), win=wins / max(len(closed), 1), skipped=skipped)


if __name__ == "__main__":
    all_coins = [f.name.replace("_1h.json", "") for f in sorted(DATA.glob("*_1h.json"))]
    big = ["BTC", "ETH", "SOL", "XRP", "HYPE", "SUI"]
    variants = [
        ("sin filtros (actual)", {}),
        ("filtro BTC", dict(btc_filter=True)),
        ("filtro funding > 2x normal", dict(funding_max=2 * BASE_FUNDING)),
        ("filtro funding > 3x normal", dict(funding_max=3 * BASE_FUNDING)),
        ("BTC + funding 2x", dict(btc_filter=True, funding_max=2 * BASE_FUNDING)),
        ("BTC + funding 3x", dict(btc_filter=True, funding_max=3 * BASE_FUNDING)),
    ]
    for label, coins in (("15 monedas", all_coins), ("solo grandes", big)):
        print(f"\n== {label} · $500 · riesgo 0,5% ==")
        print(f"{'variante':30}{'1ª ret':>9}{'DD':>7}{'2ª ret':>9}{'DD':>7}{'ops':>6}{'acierto':>9}{'filtradas':>11}")
        for name, kw in variants:
            a = run(coins, start_frac=0, end_frac=.5, **kw)
            b = run(coins, start_frac=.5, end_frac=1, **kw)
            print(f"{name:30}{100*a['ret']:>8.1f}%{100*a['mdd']:>6.1f}%{100*b['ret']:>8.1f}%{100*b['mdd']:>6.1f}%"
                  f"{a['trades'] + b['trades']:>6}{100*(a['win'] + b['win'])/2:>8.0f}%{a['skipped'] + b['skipped']:>11}")
