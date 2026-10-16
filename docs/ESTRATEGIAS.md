# Estrategias y backtests

Resumen de qué hace cada estrategia, cómo se validó y qué se descartó. Los scripts están en [`tools/backtest/`](../tools/backtest/) y usan datos públicos de Hyperliquid.

**Cómo se valida:**
- **Datos:** 208 días de velas de 1 hora y funding real de las 15 monedas con más volumen.
- **Costos:** comisión taker, slippage y funding.
- **Regla:** los parámetros se eligen en la primera mitad de los datos y se miden en la segunda, que el optimizador nunca vio. Solo se implementa lo que sale positivo en **ambas mitades**.

> ⚠️ **Sesgo de supervivencia:** las monedas se eligieron por su volumen actual, y eso favorece a las que ya subieron. Los resultados reales serán menores que los del backtest.

---

## En uso

### 🚀 Tendencia (estrategia principal)
| Regla | Valor |
|---|---|
| Entrada | El cierre de 1h supera el máximo de las 168 velas previas (7 días). Solo largos |
| Stop | 3 ATR bajo la entrada; sube con el precio (trailing). Vive en el exchange |
| Salida | Por stop, o si el cierre cae bajo el mínimo de las últimas 24 h |
| Tamaño | Riesgo fijo por operación (0,5%). Si queda bajo $10, se sube a $10 siempre que el riesgo no pase de 1,25% |
| Límites | Máximo 4 posiciones y 3x de apalancamiento total |
| **Filtro de funding** | No entra si el funding medio de 24 h de la moneda supera 2x el normal |

**Resultados (cartera de $500, riesgo 0,5%):**

| | 1ª mitad | 2ª mitad |
|---|---|---|
| Sin filtro | +9,7% (caída máx 4,5%) | +2,2% (13,3%) |
| **Con filtro de funding** | **+11,0% (3,9%)** | **+7,1% (11,6%)** |

Gana menos de la mitad de las operaciones (34–41%), pero las ganadoras valen ~2,5 veces lo que cuestan las perdedoras.

### 🔄 Rebote (complementaria, en papel)
| Regla | Valor |
|---|---|
| Entrada | RSI de 1h menor a 20 (sobreventa extrema, tras caídas fuertes de unas horas) |
| Salida | RSI mayor a 50 (rebotó), 48 h sin rebote o stop de 3 ATR |
| Tamaño | 0,5% de riesgo, máximo 2 posiciones; nunca en una moneda con posición de tendencia |

**Resultados:** +4,8R y +4,7R en las dos mitades. Fue la única forma de operar caídas que salió positiva, pero con solo ~76 operaciones. Por eso corre en papel salvo `REBOUND_LIVE=true`.

**En vivo (papel):** la primera operación fue ETH, de $2.361 a $2.438 en 12 h: **+$2,49 (+1,0R, +0,5% de la cuenta)**.

---

## Investigado y descartado

| Estrategia | Resultado | Por qué se descartó |
|---|---|---|
| **Market making** | Pierde 4–8% en 3,6 días; el apalancamiento multiplica la pérdida | El mínimo de $10, las comisiones y la selección adversa pesan más que el spread |
| **Venta en corto en caídas** | −15R y −42R | Las criptos rebotan fuerte y los stops saltan una y otra vez |
| **Comprar el retroceso en tendencia alcista** | ≈0R en mercado plano | Solo gana cuando el mercado ya sube |
| **Filtro de BTC** (comprar solo si BTC sube) | Mejora una mitad y empeora la otra | Inconsistente |
| **Rotación por momentum** (comprar las 2–3 más fuertes de la semana) | +120%/+158% con todas las monedas; **−17%** solo con las grandes | Depende de que haya altcoins que se disparen (sesgo de supervivencia) y tiene caídas de ~30%. Candidata a revisar con un universo histórico sin sesgo |
| **Patrones por hora del día** | Dirección del mercado por hora: ruido (se repite en 15 de 24 horas; al azar, ~12) | Sí hay un patrón real de **volatilidad**: más alta entre las 10 y las 13 de Chile (apertura de EE.UU.) |

**Hora de entrada de la tendencia (349 operaciones):** las entradas entre las **01 y las 05 de Chile** fueron negativas en ambas mitades (−0,26R y −0,24R, 26% de aciertos). Es un candidato a filtro, pero con solo 39 operaciones no está probado. El dashboard (Estadísticas → Resultados por hora de entrada) lo va midiendo con tus operaciones reales.

---

## Funding (cobro de tasa, no implementado)
Comprar al contado y vender en corto en el perpetuo la misma cantidad da exposición neutra y cobra el funding cada hora:
- **Rendimiento:** entre 5% y 16% anual; HYPE fue la más estable, sin semanas negativas.
- **Pro:** riesgo de mercado casi nulo.
- **Contra:** rinde poco (~$40 al año con $500) y requiere programar órdenes al contado.

## Cómo repetir los análisis
```bash
cd tools/backtest
python fetch_research.py    # datos (borra data/ para refrescarlos)
python portfolio.py         # tendencia: cartera vs buy & hold
python trend_filters.py     # filtros de BTC y funding
python dips.py              # estrategias para caídas (rebote)
python rotation.py          # rotación por momentum
python hours.py             # patrones por hora del día
python funding_stats.py     # funding por moneda
```
