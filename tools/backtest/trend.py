"""Backtest de estrategias de tendencia en velas de 1h de Hyperliquid.

Costos realistas:
- Entrada/salida a mercado: fee taker 0.045% + slippage 0.05% por lado.
- Funding REAL horario (longs pagan si funding > 0, shorts cobran).
- Stop por ATR ejecutado en el nivel (o en la apertura si hay gap).
Tamaño: riesgo fijo por operación (% del equity) según la distancia al stop,
limitado por apalancamiento máximo y mínimo de $10 por orden.

Validación: los parámetros se eligen en la primera mitad (IS) y se evalúan
en la segunda (OOS), que el optimizador nunca vio.
"""
import itertools, json, pathlib, sys

DATA = pathlib.Path(__file__).parent / "data"
TAKER, SLIP = 0.00045, 0.0005
COST = TAKER + SLIP
MIN_ORDER = 10.0


def load(coin):
    rows = json.loads((DATA / f"{coin}_1h.json").read_text())
    fund = dict(json.loads((DATA / f"{coin}_funding.json").read_text()))
    # funding indexado por hora
    fh = {}
    for t, r in fund.items():
        fh[(int(t) // 3600_000) * 3600_000] = r
    return rows, fh


def ema(prev, x, n):
    a = 2 / (n + 1)
    return x if prev is None else prev + a * (x - prev)


def simulate(rows, fh, *, kind, fast, slow, atr_n=14, stop_atr=3.0, trail=True,
             long_only=False, risk=0.01, max_lev=3.0, capital=50.0, trend_ema=0,
             start=0, end=None):
    end = end or len(rows)
    eq = capital
    pos = 0.0; entry = 0.0; stop = 0.0
    peak = capital; mdd = 0.0
    trades = []
    ef = es = et = None
    atr = None
    hist_h, hist_l = [], []
    prev_c = None
    for i in range(end):
        t, o, h, l, c, v = rows[i]
        active = i >= start

        # --- posición abierta: funding y stop ---
        if pos and active:
            r = fh.get((t // 3600_000) * 3600_000, 0.0)
            eq -= pos * o * r  # long paga funding positivo
            if pos > 0 and l <= stop:
                px = min(o, stop) * (1 - SLIP)
                eq += pos * (px - entry) - abs(pos) * px * TAKER
                trades.append(eq); pos = 0.0
            elif pos < 0 and h >= stop:
                px = max(o, stop) * (1 + SLIP)
                eq += pos * (px - entry) - abs(pos) * px * TAKER
                trades.append(eq); pos = 0.0

        # --- indicadores con datos hasta el cierre de esta vela ---
        tr = h - l if prev_c is None else max(h - l, abs(h - prev_c), abs(l - prev_c))
        atr = tr if atr is None else atr + (tr - atr) / atr_n
        prev_c = c
        ef = ema(ef, c, fast); es = ema(es, c, slow)
        if trend_ema:
            et = ema(et, c, trend_ema)
        hist_h.append(h); hist_l.append(l)
        if len(hist_h) > max(fast, slow) + 1:
            hist_h.pop(0); hist_l.pop(0)
        if i < max(slow, trend_ema, atr_n) + 2:
            continue

        # --- señal ---
        if kind == "ema":
            sig = 1 if ef > es else -1
        else:  # donchian: ruptura del máximo/mínimo de `slow` velas, salida por `fast`
            hh = max(hist_h[-slow - 1:-1]); ll = min(hist_l[-slow - 1:-1])
            ex_h = max(hist_h[-fast - 1:-1]); ex_l = min(hist_l[-fast - 1:-1])
            sig = 0
            if c > hh: sig = 1
            elif c < ll: sig = -1
            # salidas del canal corto
            if pos > 0 and c < ex_l: sig = -1 if not long_only else -2
            if pos < 0 and c > ex_h: sig = 1
        if trend_ema and sig == 1 and c < et: sig = 0
        if trend_ema and sig == -1 and c > et and pos <= 0: sig = 0
        if long_only and sig < 0:
            sig = -2  # solo cerrar

        if not active:
            continue

        # trailing stop
        if pos > 0 and trail:
            stop = max(stop, c - stop_atr * atr)
        if pos < 0 and trail:
            stop = min(stop, c + stop_atr * atr)

        want = 0 if sig == -2 else sig
        cur = 1 if pos > 0 else (-1 if pos < 0 else 0)
        if kind == "donchian" and sig == 0:
            want = cur
        if want != cur:
            if pos:  # cerrar a mercado en el cierre
                px = c * (1 - SLIP) if pos > 0 else c * (1 + SLIP)
                eq += pos * (px - entry) - abs(pos) * px * TAKER
                trades.append(eq); pos = 0.0
            if want and eq > 5:
                dist = stop_atr * atr
                notional = min(eq * risk / (dist / c), eq * max_lev)
                if notional >= MIN_ORDER:
                    px = c * (1 + SLIP) if want > 0 else c * (1 - SLIP)
                    pos = want * notional / px
                    entry = px
                    eq -= notional * TAKER
                    stop = px - dist if want > 0 else px + dist

        mtm = eq + (pos * (c - entry) if pos else 0)
        peak = max(peak, mtm)
        mdd = max(mdd, (peak - mtm) / peak if peak > 0 else 1)
        if mtm <= 0:
            return dict(ret=-1, mdd=1, n=len(trades), win=0)

    if pos:
        c = rows[end - 1][4]
        eq += pos * (c - entry) - abs(pos) * c * COST
        trades.append(eq)
    wins = sum(1 for a, b in zip([capital] + trades, trades) if b > a)
    return dict(ret=eq / capital - 1, mdd=mdd, n=len(trades), win=wins / len(trades) if trades else 0)


GRID = {
    "ema": dict(fast=[12, 24, 48], slow=[72, 120, 240], stop_atr=[2.0, 3.0, 4.0], long_only=[False, True]),
    "donchian": dict(fast=[12, 24, 48], slow=[48, 96, 168], stop_atr=[2.0, 3.0, 4.0], long_only=[False, True]),
}

if __name__ == "__main__":
    coins = [f.name.replace("_1h.json", "") for f in sorted(DATA.glob("*_1h.json"))]
    data = {c: load(c) for c in coins}
    report = []
    for kind, g in GRID.items():
        for combo in itertools.product(*g.values()):
            kw = dict(zip(g.keys(), combo))
            if kw["fast"] >= kw["slow"]:
                continue
            is_r, oos_r, oos_dd, n = [], [], [], 0
            for c, (rows, fh) in data.items():
                half = len(rows) // 2
                a = simulate(rows, fh, kind=kind, end=half, **kw)
                b = simulate(rows, fh, kind=kind, start=half, **kw)
                is_r.append(a["ret"]); oos_r.append(b["ret"]); oos_dd.append(b["mdd"]); n += b["n"]
            report.append(dict(kind=kind, kw=kw,
                               is_mean=sum(is_r) / len(is_r), oos_mean=sum(oos_r) / len(oos_r),
                               oos_pos=sum(r > 0 for r in oos_r), oos_dd=max(oos_dd), n=n))
    report.sort(key=lambda r: -r["is_mean"])
    print(f"Monedas: {len(coins)}  |  ~{len(data[coins[0]][0]) // 48} días por mitad  |  capital $50, riesgo 1%/op, máx 3x")
    print("\nTop 10 elegidos por la mitad IS -> cómo les fue en OOS (datos no vistos):")
    print(f"{'estrategia':10}{'IS media %':>11}{'OOS media %':>12}{'OOS >0':>8}{'OOS maxDD %':>12}{'ops OOS':>8}  params")
    for r in report[:10]:
        print(f"{r['kind']:10}{100 * r['is_mean']:>11.1f}{100 * r['oos_mean']:>12.1f}"
              f"{r['oos_pos']:>5}/{len(coins):<2}{100 * r['oos_dd']:>12.1f}{r['n']:>8}  {r['kw']}")
    pos_both = [r for r in report if r["is_mean"] > 0 and r["oos_mean"] > 0]
    print(f"\nPositivas en IS y OOS: {len(pos_both)} de {len(report)}")
    json.dump(report, open(DATA / "trend_report.json", "w"))
