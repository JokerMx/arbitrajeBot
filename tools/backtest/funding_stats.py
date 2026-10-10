"""Estadísticas de funding: ¿cuánto cobra un short (o un spot-perp delta-neutral)?"""
import json, pathlib, statistics as st

DATA = pathlib.Path(__file__).parent / "data"


def apr(xs):
    return 100 * sum(xs) / len(xs) * 24 * 365


print(f"{'coin':6}{'APR media':>10}{'APR mediana':>12}{'% horas >0':>11}{'APR 30d':>9}{'peor 7d':>9}{'mejor 7d':>9}")
for f in sorted(DATA.glob("*_funding.json")):
    coin = f.name.replace("_funding.json", "")
    r = [x[1] for x in json.loads(f.read_text())]
    if len(r) < 500:
        continue
    weeks = [apr(r[i:i + 168]) for i in range(0, len(r) - 168, 24)]
    print(f"{coin:6}{apr(r):>10.1f}{100 * st.median(r) * 24 * 365:>12.1f}"
          f"{100 * sum(x > 0 for x in r) / len(r):>11.0f}{apr(r[-720:]):>9.1f}"
          f"{min(weeks):>9.1f}{max(weeks):>9.1f}   ({len(r) // 24} días)")
