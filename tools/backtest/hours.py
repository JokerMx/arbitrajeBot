"""¿Hay patrones por hora del día? (UTC; Chile = UTC-3)

1. Mercado: rendimiento medio y volatilidad de cada hora (15 monedas, 208 días).
2. Estrategia de tendencia: resultado (R) de las entradas según la hora en que ocurren.
Validación: se compara la 1ª mitad con la 2ª. Un patrón real debe repetirse en ambas.
"""
import statistics as st
from datetime import datetime, timezone
from trend import load, DATA
from dips import indicators, COST

coins = [f.name.replace("_1h.json", "") for f in sorted(DATA.glob("*_1h.json"))]
hour = lambda ms: datetime.fromtimestamp(ms / 1000, tz=timezone.utc).hour

# ---------- 1. Mercado por hora ----------
ret = {h: ([], []) for h in range(24)}  # (1ª mitad, 2ª mitad)
for c in coins:
    rows, _ = load(c)
    half = len(rows) // 2
    for i, (t, o, h, l, cl, v) in enumerate(rows):
        ret[hour(t)][0 if i < half else 1].append((cl / o - 1) * 100)

print("1) MERCADO por hora UTC (15 monedas): rendimiento medio % y volatilidad %")
print(f"{'hora UTC':>8}{'Chile':>7}{'media 1ª':>10}{'media 2ª':>10}{'volat.':>9}")
for h in range(24):
    a, b = ret[h]
    print(f"{h:>8}{(h - 3) % 24:>7}{st.mean(a):>+10.3f}{st.mean(b):>+10.3f}{st.pstdev(a + b):>9.3f}")
same = sum(1 for h in range(24) if st.mean(ret[h][0]) * st.mean(ret[h][1]) > 0)
print(f"Horas cuyo signo se repite en ambas mitades: {same} de 24 (al azar se esperan ~12)")

# ---------- 2. Entradas de tendencia por hora ----------
trades = []  # (hora_entrada, R, mitad)
for c in coins:
    rows, fh = load(c)
    ind = indicators(rows)
    half = len(rows) // 2
    pos = None
    for i in range(170, len(rows)):
        b = ind[i]
        if pos:
            pos["stop"] = max(pos["stop"], b["c"] - 3 * b["atr"])
            ll = min(x["l"] for x in ind[i - 24:i])
            exit_px = None
            if b["l"] <= pos["stop"]:
                exit_px = min(b["o"], pos["stop"])
            elif b["c"] < ll:
                exit_px = b["c"]
            if exit_px is not None:
                r = ((exit_px - pos["entry"]) / pos["entry"] - COST) / pos["risk"]
                trades.append((pos["hour"], r, 0 if pos["i"] < half else 1))
                pos = None
            continue
        if b["c"] > max(x["h"] for x in ind[i - 168:i]):
            dist = 3 * b["atr"]
            pos = dict(entry=b["c"], stop=b["c"] - dist, risk=dist / b["c"], hour=hour(b["t"]), i=i)

print(f"\n2) ESTRATEGIA DE TENDENCIA: {len(trades)} operaciones, resultado medio (R) por bloque de 4 h de entrada")
print(f"{'bloque UTC':>11}{'Chile':>12}{'ops':>6}{'R 1ª':>8}{'R 2ª':>8}{'acierto':>9}")
for start in range(0, 24, 4):
    sel = [t for t in trades if start <= t[0] < start + 4]
    a = [r for _, r, m in sel if m == 0]
    b = [r for _, r, m in sel if m == 1]
    win = sum(r > 0 for _, r, _ in sel) / max(len(sel), 1)
    chile = f"{(start - 3) % 24:02d}-{(start + 1) % 24:02d}h"
    print(f"{start:>5}-{start + 4:<5}{chile:>12}{len(sel):>6}{(st.mean(a) if a else 0):>+8.2f}{(st.mean(b) if b else 0):>+8.2f}{100 * win:>8.0f}%")
print(f"Media general: {st.mean(r for _, r, _ in trades):+.2f}R por operación")
