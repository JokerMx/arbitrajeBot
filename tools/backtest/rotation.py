"""Rotación por momentum: cada R horas se ordenan las monedas por su rendimiento de
las últimas L horas y se mantienen compradas las N más fuertes (peso igual).

Filtros opcionales:
  - "abs": solo se compra una moneda si su rendimiento de L horas es positivo
  - "btc": solo se está invertido si BTC está sobre su media móvil de 7 días (168 h)
  - "abs+btc": ambos

Costos: comisión taker + slippage sobre lo que cambia en cada rebalanceo, y funding
real horario (los largos pagan funding positivo). Exposición total = 1x el capital.
Validación: los parámetros se eligen en la 1ª mitad y se miden en la 2ª.
"""
import itertools, json
from trend import load, DATA, TAKER, SLIP

COST = TAKER + SLIP
CAPITAL = 500.0


def prepare():
    coins = [f.name.replace("_1h.json", "") for f in sorted(DATA.glob("*_1h.json"))]
    data = {c: load(c) for c in coins}
    times = sorted(set.intersection(*[set(r[0] for r in data[c][0]) for c in coins]))
    close = {c: {r[0]: r[4] for r in data[c][0]} for c in coins}
    fund = {c: data[c][1] for c in coins}
    return coins, times, close, fund


def run(coins, times, close, fund, *, L, N, R, filt, start_frac=0.0, end_frac=1.0):
    t0, t1 = int(len(times) * start_frac), int(len(times) * end_frac)
    eq = CAPITAL
    w = {}  # moneda -> valor en USD de la posición
    peak, mdd, trades = CAPITAL, 0.0, 0
    btc_ema = None
    start = max(L, 168) + 1
    for k in range(1, t1):
        t, tp = times[k], times[k - 1]
        b = close["BTC"][t]
        btc_ema = b if btc_ema is None else btc_ema + (b - btc_ema) * 2 / 169
        if k < t0:
            continue
        # 1. Variación de precio y funding de la hora sobre lo que está invertido
        for c in list(w):
            ret = close[c][t] / close[c][tp] - 1
            pnl = w[c] * ret - w[c] * fund[c].get(t, 0.0)
            w[c] += pnl
            eq += pnl
        # 2. Rebalanceo cada R horas
        if k >= start and (k - start) % R == 0:
            mom = {c: close[c][t] / close[c][times[k - L]] - 1 for c in coins}
            ranked = sorted(coins, key=lambda c: -mom[c])
            pick = ranked[:N]
            if "abs" in filt:
                pick = [c for c in pick if mom[c] > 0]
            if "btc" in filt and b < btc_ema:
                pick = []
            target = {c: eq / N for c in pick}  # exposición total ≤ 1x
            for c in set(w) | set(target):
                delta = abs(target.get(c, 0.0) - w.get(c, 0.0))
                if delta > 1e-9:
                    eq -= delta * COST
                    if c not in w or c not in target:
                        trades += 1
            w = dict(target)
        peak = max(peak, eq)
        mdd = max(mdd, (peak - eq) / peak)
    bh = sum(close[c][times[t1 - 1]] / close[c][times[max(t0, start)]] - 1 for c in coins) / len(coins)
    return dict(ret=eq / CAPITAL - 1, mdd=mdd, trades=trades, bh=bh)


if __name__ == "__main__":
    coins, times, close, fund = prepare()
    days = len(times) / 24
    print(f"{len(coins)} monedas, {days:.0f} días · capital ${CAPITAL:.0f} · exposición 1x")
    grid = list(itertools.product([72, 168, 336, 720], [1, 2, 3], [24, 72, 168], ["", "abs", "btc", "abs+btc"]))
    res = []
    for L, N, R, filt in grid:
        a = run(coins, times, close, fund, L=L, N=N, R=R, filt=filt, start_frac=0, end_frac=.5)
        b = run(coins, times, close, fund, L=L, N=N, R=R, filt=filt, start_frac=.5, end_frac=1)
        res.append((a, b, dict(L=L, N=N, R=R, filt=filt or "ninguno")))

    def line(a, b, p):
        return (f"{100*a['ret']:>8.1f}%{100*a['mdd']:>7.1f}%{100*b['ret']:>9.1f}%{100*b['mdd']:>7.1f}%"
                f"{a['trades'] + b['trades']:>6}   L={p['L']}h N={p['N']} cada {p['R']}h filtro={p['filt']}")

    print(f"\nB&H promedio: 1ª mitad {100*res[0][0]['bh']:.1f}% · 2ª mitad {100*res[0][1]['bh']:.1f}%")
    hdr = f"{'1ª ret':>9}{'DD':>8}{'2ª ret':>10}{'DD':>8}{'ops':>6}   parámetros"
    print("\nTop 10 elegidas por la 1ª mitad -> resultado en la 2ª (no vista):")
    print(hdr)
    for a, b, p in sorted(res, key=lambda r: -r[0]["ret"])[:10]:
        print(line(a, b, p))
    both = [r for r in res if r[0]["ret"] > 0 and r[1]["ret"] > 0]
    print(f"\nPositivas en AMBAS mitades: {len(both)} de {len(res)} (ordenadas por la peor mitad)")
    print(hdr)
    for a, b, p in sorted(both, key=lambda r: -min(r[0]["ret"], r[1]["ret"]))[:12]:
        print(line(a, b, p))
    json.dump([(a, b, p) for a, b, p in res], open(DATA / "rotation_report.json", "w"))
