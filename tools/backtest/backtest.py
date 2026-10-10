"""Backtest aproximado del market maker sobre velas de 1m de Hyperliquid.

Supuestos (deliberadamente algo optimistas para el bot):
- Una orden límite se ejecuta si la vela opera ESTRICTAMENTE más allá de su precio
  (ignora la cola, lo que favorece al bot).
- Fees: maker 0.015%, taker 0.045%, slippage de cierre a mercado 0.02%.
- Stop por posición: ejecución en el nivel del stop + slippage (sin gaps).
- Liquidación en cross: equity < margen de mantenimiento (1/(2*maxLev) del nocional).
- Funding ignorado.
"""
import json, math, pathlib, statistics, sys

DATA = pathlib.Path(__file__).parent / "data"
MAKER, TAKER, SLIP = 0.00015, 0.00045, 0.0002
MAX_LEV = {"BTC": 40, "ETH": 25, "SOL": 20}
MIN_ORDER = 10.0


def run(coin, capital, lev, *, gamma=0.15, skew=0.5, min_spread=0.0007, stop=0.01,
        lookback=30, trend_filter=0.0, vol_mult=None, max_hold=None):
    rows = json.loads((DATA / f"{coin}.json").read_text())
    maint = 1 / (2 * MAX_LEV[coin])
    eq = capital               # equity realizada (cash)
    pos = 0.0                  # unidades con signo
    entry = 0.0
    peak, mdd = capital, 0.0
    trips = stops = fills = 0
    wins = losses = 0
    hold = 0
    closes = []
    liquidated = False

    def close_at(px, taker=True):
        nonlocal eq, pos, entry, wins, losses
        pnl = pos * (px - entry) - abs(pos) * px * (TAKER if taker else MAKER)
        eq += pnl
        wins += pnl > 0
        losses += pnl <= 0
        pos, entry = 0.0, 0.0
        return pnl

    for i, (t, o, h, l, c, v) in enumerate(rows):
        closes.append(o)
        if len(closes) > lookback + 1:
            closes.pop(0)

        # --- liquidación (peor precio de la vela) ---
        if pos:
            worst = l if pos > 0 else h
            if eq + pos * (worst - entry) < maint * abs(pos) * worst:
                eq = 0.0
                liquidated = True
                break

        # --- stop por posición ---
        if pos and stop:
            stop_px = entry * (1 - stop) if pos > 0 else entry * (1 + stop)
            hit = (l <= stop_px) if pos > 0 else (h >= stop_px)
            if hit:
                px = stop_px * (1 - SLIP) if pos > 0 else stop_px * (1 + SLIP)
                close_at(px)
                stops += 1
                hold = 0
                continue

        if max_hold and pos:
            hold += 1
            if hold >= max_hold:
                close_at(o * (1 - SLIP) if pos > 0 else o * (1 + SLIP))
                stops += 1
                hold = 0
                continue

        if len(closes) < 5 or eq < 1:
            continue

        rets = [(b - a) / a for a, b in zip(closes, closes[1:])]
        sigma = statistics.pstdev(rets)
        mid = o
        mult = vol_mult if vol_mult is not None else 0.5 + 5 * gamma
        half = min(max(sigma * mult, min_spread / 2), 0.01)

        max_pos_usd = eq * lev
        q = max(-1, min(1, pos * mid / max_pos_usd)) if max_pos_usd > 0 else 0
        res = mid * (1 - q * skew * half * 2)
        bid = min(res * (1 - half), mid * (1 - MAKER))
        ask = max(res * (1 + half), mid * (1 + MAKER))

        order_usd = max(max_pos_usd * 0.25, MIN_ORDER)
        size = order_usd / mid
        can_add = abs(pos) * mid + order_usd <= max(max_pos_usd, MIN_ORDER)
        # no se puede abrir si el margen inicial no alcanza
        if order_usd / max(lev, 1) > eq:
            can_add = False

        # filtro de tendencia: no abrir contra el movimiento reciente
        mom = (closes[-1] - closes[-6]) / closes[-6] if len(closes) >= 6 else 0
        allow_buy = allow_sell = True
        if trend_filter:
            if mom < -trend_filter * sigma * math.sqrt(5):
                allow_buy = False
            if mom > trend_filter * sigma * math.sqrt(5):
                allow_sell = False

        buy = sell = None
        if pos > 0:
            sell = (abs(pos), max(ask, entry * (1 + 2 * MAKER)), True)
            if can_add and allow_buy:
                buy = (size, bid, False)
        elif pos < 0:
            buy = (abs(pos), min(bid, entry * (1 - 2 * MAKER)), True)
            if can_add and allow_sell:
                sell = (size, ask, False)
        else:
            if can_add and allow_buy:
                buy = (size, bid, False)
            if can_add and allow_sell:
                sell = (size, ask, False)

        # --- ejecuciones (orden: lado que la vela toca primero según dirección) ---
        sides = [("b", buy), ("s", sell)] if c < o else [("s", sell), ("b", buy)]
        for name, od in sides:
            if not od:
                continue
            sz, px, red = od
            if name == "b" and l < px:
                sz = min(sz, abs(pos)) if red else sz
                if pos < 0:
                    closed = min(sz, -pos)
                    pnl = -closed * (px - entry) - closed * px * MAKER
                    eq += pnl; wins += pnl > 0; losses += pnl <= 0
                    pos += closed; trips += 1
                    if pos == 0: entry = 0.0
                    sz -= closed
                if sz > 0:
                    eq -= sz * px * MAKER
                    entry = (entry * pos + px * sz) / (pos + sz) if pos > 0 else px
                    pos += sz
                fills += 1
                hold = 0
            elif name == "s" and h > px:
                sz = min(sz, abs(pos)) if red else sz
                if pos > 0:
                    closed = min(sz, pos)
                    pnl = closed * (px - entry) - closed * px * MAKER
                    eq += pnl; wins += pnl > 0; losses += pnl <= 0
                    pos -= closed; trips += 1
                    if pos == 0: entry = 0.0
                    sz -= closed
                if sz > 0:
                    eq -= sz * px * MAKER
                    entry = (entry * -pos + px * sz) / (-pos + sz) if pos < 0 else px
                    pos -= sz
                fills += 1
                hold = 0

        mtm = eq + (pos * (c - entry) if pos else 0)
        peak = max(peak, mtm)
        mdd = max(mdd, (peak - mtm) / peak if peak else 0)

    final = eq + (pos * (rows[-1][4] - entry) if pos and not liquidated else 0)
    days = len(rows) / 1440
    return dict(coin=coin, lev=lev, final=final, ret=final / capital - 1, mdd=mdd,
                trips=trips, stops=stops, wins=wins, losses=losses,
                liq=liquidated, days=days)


def show(title, results):
    print(f"\n== {title} ==")
    print(f"{'coin':5}{'lev':>5}{'final $':>10}{'ret %':>9}{'maxDD %':>9}{'cierres+':>9}{'stops':>7}{'win%':>7}{'liq':>5}")
    for r in results:
        tot = r['wins'] + r['losses']
        wr = 100 * r['wins'] / tot if tot else 0
        print(f"{r['coin']:5}{r['lev']:>5}{r['final']:>10.2f}{100*r['ret']:>9.1f}{100*r['mdd']:>9.1f}"
              f"{r['trips']:>9}{r['stops']:>7}{wr:>7.0f}{'SI' if r['liq'] else '':>5}")


if __name__ == "__main__":
    CAP = 19.0
    coins = ["BTC", "ETH", "SOL"]
    days = run("BTC", CAP, 1)["days"]
    print(f"Capital ${CAP}, datos: {days:.1f} días de velas 1m por moneda")
    for lev in (1, 3, 5, 10, 20):
        show(f"Estrategia actual, apalancamiento efectivo {lev}x",
             [run(c, CAP, lev) for c in coins])
