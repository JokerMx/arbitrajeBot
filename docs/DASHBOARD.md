# Guía del dashboard

Dirección: **http://localhost:3001/dashboard/**. Desde otro equipo, abre un túnel SSH hacia el servidor (ver [PUESTA_EN_MARCHA.md](../PUESTA_EN_MARCHA.md)).

La primera vez pide el **token de administración** (`ADMIN_API_TOKEN` del `.env`) y lo recuerda en ese navegador.

Todos los datos de operaciones salen del **historial de señales** del bot, que se graba siempre, con o sin Telegram.

## Encabezado

| Elemento | Qué hace |
|---|---|
| Punto y texto de estado | 🟢 **Operando**, 🟡 **En pausa** o 🔴 **Bot sin conexión** |
| Etiqueta **PAPEL / REAL** | PAPEL = simulación sin órdenes reales. REAL = dinero real |
| 🔔 Campana | Lleva a **Alertas**; el número indica las señales nuevas desde la última visita |
| ⚙️ Engranaje | Lleva a **Configuración** |
| ⏻ Botón de encendido | Abre el menú de control (ver abajo) |

**Menú de control**
- **Pausar nuevas entradas:** el bot deja de abrir posiciones. Las abiertas **se siguen gestionando** con su stop y su salida. La pausa sobrevive a reinicios.
- **Reanudar entradas:** vuelve a abrir posiciones cuando haya señales.
- **Reiniciar bot:** el proceso se reinicia y vuelve en unos 30 segundos. Úsalo después de cambiar el capital inicial o la configuración de Telegram.

## Dashboard (página principal)

- **Tarjetas:**
  - **Equity:** capital actual.
  - **P&L total:** lo realizado más lo abierto.
  - **Aciertos:** % de operaciones ganadoras.
  - **Profit factor:** ganancias divididas por pérdidas.
  - **R medio:** ganancia promedio en múltiplos del riesgo arriesgado.
  - **Caída máxima:** peor retroceso del capital.
- **Evolución del balance:**
  - una foto **cada 5 minutos**, que incluye las posiciones abiertas a precio actual;
  - cada cierre aparece como un punto verde (ganancia) o rojo (pérdida);
  - rangos de 24 h, 3 días, 7 días o 30 días;
  - la curva **parte del capital inicial** (línea punteada) y sube o baja con cada cierre, así siempre se ve la evolución completa; la escala se ajusta para que se aprecien incluso cambios de centavos;
  - arriba muestra cuánto cambió el balance en el período elegido. Si en ese rango no hubo posiciones abiertas, la línea es plana: es lo correcto;
  - el historial se guarda en `equity_history.json`: fotos de 5 min de los últimos 7 días y una por hora hasta 1 año.
- **Por estrategia:** resultado de tendencia y de rebote por separado.
- **Posiciones abiertas:** entrada, precio actual, stop (con ↑ si ya se subió), tamaño, P&L y **% de la cuenta**.
- **Últimas operaciones:** las 10 más recientes (el historial completo está en **Historial**).

### Cómo leer una operación

Ejemplo: `$76,47 · +$2,49 (+3,25% precio) · +0,50% cuenta · +1,00R`

- **$76,47:** tamaño de la posición. El bot lo calcula para que, si salta el stop, pierdas solo el riesgo configurado (0,5% de la cuenta).
- **+3,25% precio:** cuánto se movió el precio entre la entrada y la salida.
- **+0,50% cuenta:** lo que ganó o perdió **tu cuenta completa**. Es el número que importa.
- **+1,00R:** ganó lo mismo que arriesgaba. 2R significa el doble de lo arriesgado; −1R, que saltó el stop.

## Estadísticas

Mismo gráfico en grande y una **tabla de métricas** (total, tendencia y rebote):
- aciertos;
- ganancia y pérdida media;
- profit factor;
- R medio y R total;
- mejor y peor operación;
- duración media;
- caída máxima.

**Resultados por hora de entrada (hora de Chile):**
- tabla con operaciones, aciertos, R medio y P&L por bloques de 4 horas;
- sirve para detectar si hay horarios que conviene evitar, pero hace falta al menos ~30 operaciones por bloque para sacar conclusiones.

## Estrategia

Parámetros de **tendencia** y **rebote**:
- Se guardan en la base de datos y **se aplican en la siguiente evaluación horaria, sin reiniciar**.
- Si un valor difiere del `.env`, se muestra el valor original al lado.
- **Usar .env** borra los ajustes del dashboard.
- Hay límites de seguridad: el riesgo por operación no supera 2%, el RSI de entrada no baja de 5, etc.
- **Filtro de funding** (activo por defecto): no compra cuando el funding de 24 h de la moneda supera 2 veces el normal, porque el mercado está sobrecomprado. Mejoró ambas mitades del backtest.
- **El paso a dinero real no se puede activar desde aquí.**

La sección plegada "Market making" solo aplica con `STRATEGY_MODE=market_making`.

## Riesgo

- **Pérdida diaria máxima, drawdown máximo y kill switch:** se aplican **al instante** y se guardan. Si se superan, el bot cierra las posiciones y deja de abrir nuevas.
- **Protección de capital** (stop loss de la cuenta, capital en riesgo): también se aplica al instante.
- **Apalancamiento por símbolo** (plegado): solo market making. La tendencia usa el apalancamiento total de la sección **Estrategia**.

## Mercados

- **Monedas de la estrategia de tendencia:**
  - busca entre los ~230 mercados de Hyperliquid y agrega o quita monedas;
  - se aplica sin reiniciar;
  - si quitas una moneda con posición abierta, el bot la sigue gestionando hasta cerrarla;
  - **Usar .env** vuelve a `TREND_SYMBOLS`.
- **Mercados (market making):** solo aplica a esa estrategia.

## Historial

Todas las operaciones cerradas, con filtro por estrategia y botón **CSV** para abrirlas en Excel. También muestra las posiciones abiertas.

## Alertas

Línea de tiempo de todas las señales:
- 🚀 entrada de tendencia;
- 🔄 entrada de rebote;
- 🛡️ stop subido;
- ✅ / 🔻 cierre con su resultado.

Se actualiza cada 30 segundos. Las mismas señales llegan por Telegram.

## Usuarios

Usuarios de Telegram:
- **plan:** Free o Premium y fecha de vencimiento;
- **pagos en Stars:** con el `charge_id` para reembolsar;
- **acciones:** regalar o quitar días Premium, autorizar y suscribir;
- **broadcast:** a todos, solo Free o solo Premium;
- **métricas:** ingresos, usuarios nuevos y rendimiento de las señales.

## Configuración

- **Sistema** (solo lectura):
  - modo real o papel de cada estrategia;
  - exchange, red y wallet (enmascarada);
  - tiempo activo, versión, carpeta de datos y si hay token de la API.
  - **Capital inicial:** editable. Se aplica al reiniciar, con el botón que aparece al guardar.
- **Telegram:**
  - token, chat y tipos de aviso; se aplican al reiniciar;
  - el token se muestra enmascarado; déjalo así para no cambiarlo;
  - **Probar** te envía un mensaje **solo a ti**.

## Qué no se puede hacer desde el dashboard (a propósito)

- **Pasar a dinero real:** se hace en el `.env` (`EXCHANGE_TYPE=hyperliquid_mainnet`, `REBOUND_LIVE=true`) y reiniciando el bot.
- **Cerrar posiciones a mano:** se hace en app.hyperliquid.xyz. El stop de cada posición vive en el exchange.
