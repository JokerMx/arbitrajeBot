# ArbitrajeBot

Bot de trading para **Hyperliquid** escrito en Rust, con dashboard web y un bot de Telegram que vende **señales Premium** cobradas con Telegram Stars.

> ⚠️ Software experimental. Puede operar con dinero real y los backtests no garantizan resultados. Lee [PUESTA_EN_MARCHA.md](PUESTA_EN_MARCHA.md) antes de usarlo en producción.

## Qué hace

| Pieza | Descripción |
|---|---|
| **Estrategia de tendencia** (principal) | Velas de 1h. Compra cuando el precio rompe su máximo de 7 días, solo en largo. Stop de 3 ATR que sube con el precio y vive en el propio exchange. Arriesga un % fijo del capital por operación |
| **Estrategia de rebote** (complementaria) | Compra en sobreventa extrema (RSI 1h < 20) y vende al rebotar (RSI > 50) o a las 48 h. Opera en caídas, cuando la tendencia está quieta. **Siempre en papel** salvo `REBOUND_LIVE=true` |
| **Modo papel** | Con `EXCHANGE_TYPE=simulated` calcula las mismas señales con datos reales, pero no envía órdenes |
| **Dashboard web** | Operaciones, estadísticas, control del bot y configuración en vivo. Guía en [docs/DASHBOARD.md](docs/DASHBOARD.md) |
| **Telegram** | Free recibe aviso inmediato y detalle con retraso; Premium, la señal al instante. Prueba gratis, referidos, pagos con Stars y panel de administración |
| **Market making** (no recomendado) | Avellaneda-Stoikov, disponible con `STRATEGY_MODE=market_making`. No mostró ventaja en los backtests |

Cómo funcionan las estrategias y qué dijeron los backtests: [docs/ESTRATEGIAS.md](docs/ESTRATEGIAS.md).

---

## Probar en tu PC (modo papel, con Docker)

No necesitas Rust. Este entorno **no usa tu `.env` real**, no envía órdenes y crea su propia base de datos.

```bash
./probar.sh            # prepara .env.test, compila el dashboard si cambió, levanta todo y verifica la API
./probar.sh logs       # seguir el log
./probar.sh reset      # tras poner/cambiar TELEGRAM_BOT_TOKEN en .env.test
./probar.sh stop       # detener
```

| Qué probar | Cómo |
|---|---|
| Dashboard | http://localhost:3001/dashboard/ (token: `test-local`) |
| API sin token → 401 | `curl -i http://localhost:3001/api/bot/status` |
| API con token | `curl -H "Authorization: Bearer test-local" http://localhost:3001/api/journal` |
| Telegram (con un bot de **prueba** de @BotFather) | `/start`, `/trial`, `/signals`, `/performance`, `/premium` (cuesta 1 ⭐ en `.env.test`) |
| Ver entradas rápido | En `.env.test` descomenta `TREND_BREAKOUT_HOURS=24` y `TREND_EXIT_HOURS=6` |

> `probar.sh` usa el puerto 3001, igual que el bot de producción en Docker: no pueden correr a la vez.
> El token de Telegram se guarda en la base de datos la primera vez; si lo cambias, usa `./probar.sh reset`.

## Producción

### Con Docker (recomendado)

```bash
cp .env-example .env          # completar (ver tabla de abajo)
docker compose --profile db up -d --build     # bot + MariaDB incluida
docker compose up -d --build                  # solo el bot, con tu propia DATABASE_URL
docker compose logs -f bot
```

- **Imagen:** se construye por etapas (Node → Rust → Debian slim, ~165 MB), corre sin root, se reinicia sola y guarda los datos en el volumen `botdata`.
- **Aplicar cambios del `.env`:** `docker compose up -d bot`. `restart` no vuelve a leer el `.env`.
- **Aplicar cambios de código:** `docker compose up -d --build bot`.

### Sin Docker (systemd)

```bash
cp .env-example .env && chmod 600 .env
cargo build --release --locked && cargo test --release --locked
(cd web_dashboard && npm ci && npm run build)
sudo cp deploy/arbitrajebot.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now arbitrajebot
```

Para actualizar: `./deploy/update.sh`. Para respaldar: `./deploy/backup.sh`. Los detalles, la seguridad y el checklist de marcha blanca están en [PUESTA_EN_MARCHA.md](PUESTA_EN_MARCHA.md).

## Configuración (`.env`)

Lo marcado con 🖥️ también se puede cambiar desde el dashboard: se guarda en la base de datos y tiene prioridad sobre el `.env`.

| Variable | Para qué sirve |
|---|---|
| `EXCHANGE_TYPE` | `simulated` (papel) o `hyperliquid_mainnet` (**dinero real**). Solo se cambia aquí, nunca desde la web |
| `PRIVATE_KEY` / `WALLET` | Clave que firma (idealmente una API wallet sin permiso de retiro) y cuenta principal |
| `INITIAL_CAPITAL` 🖥️ | Capital base en papel (se aplica al reiniciar) |
| `STRATEGY_MODE` | `trend` (por defecto) o `market_making` |
| `TREND_SYMBOLS` 🖥️ | Monedas, p. ej. `BTC,ETH,SOL` |
| `TREND_RISK_PER_TRADE` 🖥️ | % del capital arriesgado por operación (0.005 = 0,5%; máximo 2%) |
| `TREND_MAX_RISK_PER_TRADE` 🖥️ | Tope de riesgo al subir una posición chica al mínimo de $10 (1,25%) |
| `TREND_MAX_POSITIONS` / `TREND_MAX_LEVERAGE` 🖥️ | Límites de cartera |
| `REBOUND_ENABLED` 🖥️ / `REBOUND_LIVE` | Estrategia de rebote; `REBOUND_LIVE=true` la pasa a real (solo por `.env`) |
| `REBOUND_RISK_PER_TRADE`, `REBOUND_MAX_POSITIONS` 🖥️ | Riesgo (0,5%) y posiciones máximas (2) del rebote |
| `ADMIN_API_TOKEN` | Token de la API y del dashboard. **Obligatorio en producción** (`openssl rand -hex 32`) |
| `API_BIND` / `API_PORT` | Dirección y puerto de la API (`127.0.0.1:3001` por defecto) |
| `DATA_DIR` | Carpeta de usuarios, pagos, historial de señales y foto de equity |
| `TELEGRAM_BOT_TOKEN` / `TELEGRAM_CHAT_ID` 🖥️ | Bot de Telegram y tu chat de administrador |
| `PREMIUM_PRICE_STARS`, `TRIAL_DAYS`, `REFERRAL_BONUS_DAYS`, `FREE_SIGNAL_DELAY_HOURS` | Monetización |
| `PAYSUPPORT_CONTACT` | Contacto para `/paysupport`; Telegram lo exige en bots que cobran |

La lista completa está en [.env-example](.env-example).

## API

Todas las rutas `/api` exigen `Authorization: Bearer <ADMIN_API_TOKEN>`.

| Ruta | Uso |
|---|---|
| `GET /api/bot/status` | Estado: operando/pausa, papel/real, tiempo activo, riesgo |
| `POST /api/bot/stop` · `/api/bot/start` | Pausar / reanudar **nuevas entradas** (las posiciones abiertas se siguen gestionando) |
| `POST /api/bot/restart` | Reinicia el proceso; Docker o systemd lo vuelven a levantar |
| `GET /api/journal` | Diario: operaciones cerradas y abiertas, estadísticas, curva de equity (foto cada 5 min) |
| `GET /api/signals` · `/api/signals/performance` | Historial de señales y rendimiento por estrategia |
| `GET/PUT/DELETE /api/strategy/config` | Parámetros de tendencia y rebote (se aplican sin reiniciar) |
| `GET/PUT/DELETE /api/trend/symbols` | Monedas de la estrategia |
| `GET/POST /api/config` | Riesgo, protección, Telegram y market making |
| `GET /api/system` · `PUT /api/system/capital` | Estado del sistema y capital inicial |
| `/api/telegram/*` | Usuarios, Premium, pagos, broadcast, mensaje de prueba |

## Comandos de Telegram

- **Usuarios:** `/start`, `/premium`, `/trial`, `/plan`, `/signals` (Premium), `/performance`, `/referral`, `/subscribe`, `/unsubscribe`, `/terms`, `/paysupport`.
- **Administración:** `/status`, `/balance`, `/trades`, `/stats`, `/users`, `/authorize`, `/deauthorize`, `/grant <id> <días>`, `/revoke <id>`, `/refund <id> <charge_id>`, `/broadcast`, `/broadcast_free`, `/broadcast_premium`.

## Tests y backtests

```bash
# Tests (Rust >= 1.91)
cargo test --locked

# ...o con Docker en Windows (Git Bash)
MSYS_NO_PATHCONV=1 docker run --rm -v "$(pwd -W):/app" -v arbibot-target:/target \
  -v arbibot-cargo:/usr/local/cargo/registry -e CARGO_TARGET_DIR=/target -w /app rust:1 cargo test --locked

# Backtests con datos públicos de Hyperliquid (Python 3, sin dependencias)
cd tools/backtest
python fetch_research.py   # descarga velas 1h + funding (15 monedas con más volumen)
python trend.py            # variantes de tendencia, validación en dos mitades
python portfolio.py        # cartera completa vs buy & hold
python dips.py             # operar caídas: corto, retroceso, sobreventa (rebote)
python trend_filters.py    # filtros de BTC y de funding sobre la tendencia
python rotation.py         # rotación por momentum
python funding_stats.py    # funding anualizado por moneda
```

## Estructura

```
src/
  main.rs              arranque, BD, API, Telegram, foto de equity cada 5 min
  bot.rs               orquestador (elige estrategia)
  trend.rs             tendencia + rebote (+ modo papel, pausa, config en vivo)
  signals.rs           señales, historial, diario de operaciones y rendimiento
  risk.rs              límites de riesgo sincronizados con la equity real
  telegram_bot.rs      bot multiusuario, planes, pagos con Stars
  notifier.rs          avisos al admin y bus de señales
  market.rs strategy.rs  market making (Avellaneda-Stoikov)
  hyperliquid/         cliente (órdenes, stops, posiciones)
  api/                 REST + WebSocket (warp) con token
  db/                  configuración en MariaDB
web_dashboard/         React + Vite (se sirve en /dashboard)
tools/backtest/        investigación y validación de estrategias
deploy/                systemd, actualización y respaldo
docs/                  guía del dashboard y de las estrategias
```

## Requisitos

- Rust **≥ 1.91** (lo exige alloy vía hypersdk), o Docker.
- Node.js ≥ 20 para el dashboard.
- MariaDB/MySQL; las tablas se crean solas al arrancar.
- En Windows con Docker Desktop: al menos 8 GB asignados a Docker (`.wslconfig`) para compilar sin bloqueos.
