# ArbitrajeBot

Bot de trading para **Hyperliquid** escrito en Rust. Incluye un dashboard web y un bot de Telegram que vende **señales Premium** cobradas con Telegram Stars.

- **Estrategia por defecto:** seguimiento de tendencia en velas de 1h. Entra cuando el precio rompe el máximo de 7 días, solo en largo, con un stop de 3 ATR que sube con el precio y vive en el propio exchange. Arriesga un porcentaje fijo del capital por operación. El market making sigue disponible (`STRATEGY_MODE=market_making`), aunque en los backtests no mostró ventaja.
- **Modo papel:** si `EXCHANGE_TYPE` no es `hyperliquid_mainnet`, el bot calcula las mismas señales con datos reales pero no envía órdenes.
- **Telegram:** los usuarios Free reciben un aviso inmediato y el detalle con retraso. Premium recibe la señal al instante. Incluye prueba gratis, referidos, historial de resultados y panel de administración.

> ⚠️ Software experimental que opera con dinero real. Los backtests no garantizan resultados. Lee [PUESTA_EN_MARCHA.md](PUESTA_EN_MARCHA.md) antes de usarlo en producción.

---

## Probar en 5 minutos (modo papel, con Docker)

No necesitas Rust instalado. Este entorno **no usa tu `.env` real**, no envía órdenes y crea su propia base de datos desechable.

**Atajo, un solo comando** (Git Bash o Linux): prepara `.env.test`, compila el dashboard si cambió, levanta todo, espera a que el bot esté listo y verifica la API.

```bash
./probar.sh            # levantar y verificar
./probar.sh logs       # seguir el log
./probar.sh reset      # tras poner/cambiar TELEGRAM_BOT_TOKEN en .env.test
./probar.sh stop       # detener
```

Paso a paso, si prefieres hacerlo a mano:

```bash
# 1. Variables de prueba
cp .env.test.example .env.test
#    - PRIVATE_KEY: genera una clave desechable sin fondos:
python -c "import secrets;print('0x'+secrets.token_hex(32))"
#    - Opcional: TELEGRAM_BOT_TOKEN de un bot de PRUEBA (@BotFather) y tu TELEGRAM_CHAT_ID (@userinfobot)

# 2. Compilar el dashboard (una vez, o cuando cambie)
cd web_dashboard && npm install && npm run build && cd ..

# 3. Levantar BD + bot (la primera compilación tarda ~15-20 min; después, segundos)
docker compose -f docker-compose.test.yml up -d
docker compose -f docker-compose.test.yml logs -f bot
```

Cuando el log muestre `📊 Tendencia: equity $50.00 ...`, el bot está funcionando:

| Qué probar | Cómo |
|---|---|
| Dashboard | http://localhost:3001/dashboard/ (token: `test-local`) |
| API sin token → 401 | `curl -i http://localhost:3001/api/stats` |
| API con token | `curl -H "Authorization: Bearer test-local" http://localhost:3001/api/stats` |
| Rendimiento de señales | `curl -H "Authorization: Bearer test-local" http://localhost:3001/api/signals/performance` |
| Telegram (si configuraste el bot de prueba) | `/start`, `/trial`, `/signals`, `/performance`, `/premium` |
| Pago real barato | `/premium` cuesta 1 ⭐ en `.env.test`. Después, `/refund <tu_id> <charge_id>` (el charge_id aparece en la tabla de pagos del dashboard) |
| Ver entradas rápido | En `.env.test` descomenta `TREND_BREAKOUT_HOURS=24` y `TREND_EXIT_HOURS=6`, luego `docker compose -f docker-compose.test.yml up -d` |

Para detener el entorno:

```bash
docker compose -f docker-compose.test.yml down      # conserva BD y datos de prueba
docker compose -f docker-compose.test.yml down -v   # borra todo lo de prueba
```

> El token de Telegram se guarda en la base de datos la primera vez que arranca el bot. Si después lo cambias en `.env.test`, borra la BD de prueba con `down -v` para que se aplique.

## Tests y backtests

```bash
# Tests unitarios (con Rust >= 1.91 instalado)
cargo test --locked

# ...o con Docker, sin instalar Rust
docker run --rm -v "$PWD:/app" -v arbibot-target:/target -v arbibot-cargo:/usr/local/cargo/registry \
  -e CARGO_TARGET_DIR=/target -w /app rust:1 cargo test --locked

# Backtests con datos públicos de Hyperliquid (Python 3, sin dependencias)
cd tools/backtest
python fetch_research.py        # velas 1h + funding de las 15 monedas con más volumen
python trend.py                 # estrategias de tendencia con validación en dos mitades
python portfolio.py             # simulación de una cuenta de $50 vs buy & hold
python funding_stats.py         # funding anualizado por moneda
```

En Windows con Git Bash, antepón `MSYS_NO_PATHCONV=1` al comando `docker run`.

## Producción

Resumen (los detalles, el checklist de marcha blanca y la seguridad están en [PUESTA_EN_MARCHA.md](PUESTA_EN_MARCHA.md)):

```bash
cp .env-example .env && chmod 600 .env       # completar
cargo build --release --locked && cargo test --release --locked
(cd web_dashboard && npm ci && npm run build)
sudo cp deploy/arbitrajebot.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now arbitrajebot
journalctl -u arbitrajebot -f
```

Para actualizar: `./deploy/update.sh`. Para respaldar los datos: `./deploy/backup.sh`. Para el desarrollo diario sigue disponible `./run.sh`.

## Configuración principal (`.env`)

| Variable | Para qué sirve |
|---|---|
| `EXCHANGE_TYPE` | `simulated` (papel) o `hyperliquid_mainnet` (dinero real) |
| `PRIVATE_KEY` / `WALLET` | Clave que firma (idealmente una API wallet sin permiso de retiro) y cuenta principal |
| `STRATEGY_MODE` | `trend` (por defecto) o `market_making` |
| `TREND_SYMBOLS` | Monedas, p. ej. `BTC,ETH,SOL`. Si se omite, usa los mercados habilitados en el dashboard |
| `TREND_RISK_PER_TRADE` | Fracción del capital arriesgada por operación (0.005 = 0,5%; máximo 2%) |
| `TREND_MAX_POSITIONS` / `TREND_MAX_LEVERAGE` | Límites de cartera |
| `ADMIN_API_TOKEN` | Token de la API y del dashboard. **Obligatorio en producción** |
| `API_BIND` / `API_PORT` | Dirección y puerto de la API (`127.0.0.1:3001` por defecto) |
| `DATA_DIR` | Carpeta de `subscribers.json`, `payments.json` y `signals_history.json` |
| `PREMIUM_PRICE_STARS`, `TRIAL_DAYS`, `REFERRAL_BONUS_DAYS`, `FREE_SIGNAL_DELAY_HOURS` | Monetización |
| `PAYSUPPORT_CONTACT` | Contacto para `/paysupport`; Telegram lo exige en bots que cobran |

La lista completa está en [.env-example](.env-example).

## Comandos de Telegram

**Usuarios:** `/start`, `/premium`, `/trial`, `/plan`, `/signals` (Premium), `/performance`, `/referral`, `/subscribe`, `/unsubscribe`, `/terms`, `/paysupport`.

**Administración:** `/status`, `/balance`, `/trades`, `/stats`, `/users`, `/authorize`, `/deauthorize`, `/grant <id> <días>`, `/revoke <id>`, `/refund <id> <charge_id>`, `/broadcast`, `/broadcast_free`, `/broadcast_premium`.

## Estructura

```
src/
  main.rs              arranque, BD, API, Telegram
  bot.rs               orquestador (elige estrategia)
  trend.rs             estrategia de tendencia (+ modo papel)
  market.rs strategy.rs  market making (Avellaneda-Stoikov)
  risk.rs              límites de riesgo sincronizados con la equity real
  signals.rs           señales, historial y rendimiento
  telegram_bot.rs      bot multiusuario, planes, pagos con Stars
  notifier.rs          avisos al admin y bus de señales
  hyperliquid/         cliente (órdenes, stops, posiciones)
  api/                 REST + WebSocket (warp) con token
  db/                  configuración en MariaDB
web_dashboard/         React + Vite (se sirve en /dashboard)
tools/backtest/        investigación y validación de estrategias
deploy/                systemd, actualización y respaldo
```

## Requisitos

- Rust **≥ 1.91** (lo exige alloy vía hypersdk), o Docker.
- Node.js ≥ 20 para el dashboard.
- MariaDB/MySQL; las tablas se crean solas al arrancar.
