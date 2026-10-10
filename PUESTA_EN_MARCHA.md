# Puesta en marcha: marcha blanca → producción

El bot hace dos cosas:

1. **Trading** en Hyperliquid con la estrategia de tendencia (`STRATEGY_MODE=trend`).
2. **Señales por Telegram**: Free con retraso y Premium al instante, pagado con Telegram Stars.

La marcha blanca usa el modo **papel**: genera señales reales de mercado sin enviar órdenes, y los suscriptores las reciben marcadas como "(papel)".

---

## 1. Requisitos del servidor (Linux)

| Componente | Versión | Nota |
|---|---|---|
| Rust | **≥ 1.91** | `rustup update stable`. Con versiones anteriores, alloy/hypersdk no compila |
| Node.js | ≥ 20 | Solo para compilar el dashboard |
| MariaDB/MySQL | cualquiera reciente | Las tablas se crean solas al arrancar |
| Paquetes | `build-essential pkg-config libssl-dev` | Para `native-tls` |

```bash
sudo useradd -r -m -d /opt/arbitrajeBot arbibot
sudo -u arbibot git clone <repo> /opt/arbitrajeBot
cd /opt/arbitrajeBot
cp .env-example .env && chmod 600 .env
mkdir -p data logs backups
```

## 2. Configurar `.env`

Variables obligatorias:

- `DATABASE_URL`: un usuario propio de la BD con contraseña (no `root`).
- `TELEGRAM_BOT_TOKEN` y `TELEGRAM_CHAT_ID`: tu chat queda como administrador.
- `ADMIN_API_TOKEN`: genéralo con `openssl rand -hex 32`. Sin él, la API queda abierta.
- `PAYSUPPORT_CONTACT`: tu @usuario. Telegram lo exige en bots que cobran.
- `DATA_DIR=./data`: si existía `subscribers.json` en la raíz, se migra solo.

Para la clave de Hyperliquid, usa una **API wallet** en lugar de la clave de tu wallet principal:

1. En app.hyperliquid.xyz → *More → API*, crea una API wallet.
2. Pon su clave en `PRIVATE_KEY`.
3. Pon la dirección de tu **cuenta principal** en `WALLET`.

Una API wallet puede operar pero **no puede retirar fondos**. Si te roban el servidor, no pueden vaciar la cuenta.

## 3. Compilar y verificar

```bash
cargo build --release --locked
cargo test --release --locked          # deben pasar todos
(cd web_dashboard && npm ci && npm run build)
```

## 4. Instalar como servicio

```bash
sudo cp deploy/arbitrajebot.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now arbitrajebot
journalctl -u arbitrajebot -f
```

Respaldo diario de usuarios, pagos y señales (`crontab -e` con el usuario `arbibot`):

```
0 4 * * * /opt/arbitrajeBot/deploy/backup.sh >> /opt/arbitrajeBot/logs/backup.log 2>&1
```

El dashboard escucha solo en `127.0.0.1:3001`, y así debe seguir. Para usarlo desde tu PC, abre un túnel SSH:

```bash
ssh -L 3001:localhost:3001 usuario@servidor
# luego abre http://localhost:3001/dashboard/ ; pedirá el ADMIN_API_TOKEN una vez
```

---

## 5. Marcha blanca (2 a 4 semanas)

> Antes de tocar el servidor puedes probar todo en tu PC con `docker-compose.test.yml`. Los pasos están en el [README](README.md#probar-en-5-minutos-modo-papel-con-docker).

En `.env`:

```
EXCHANGE_TYPE=simulated
STRATEGY_MODE=trend
TREND_RISK_PER_TRADE=0.005
```

En el log debe aparecer `Estrategia TENDENCIA (PAPEL)`. Durante este periodo:

- [ ] El servicio corre 7 días seguidos sin reinicios inesperados (`systemctl status`).
- [ ] Cada hora aparece `📊 Tendencia: equity ...` en el log.
- [ ] Llegan señales de entrada, de stop movido y de cierre a Telegram, coherentes con el gráfico.
- [ ] `/performance` en Telegram y la tarjeta "Señales cerradas" del dashboard coinciden.
- [ ] Invitas 5–20 personas de confianza y les das Premium gratis con `/grant <id> 30` o el botón 🎁.
- [ ] **Pruebas de pago con Stars reales:** compra `/premium` desde una segunda cuenta de Telegram. Verifica que se activa, que aparece en "Pagos" del dashboard, y que `/refund <id> <charge_id>` devuelve las Stars y quita Premium.
- [ ] Pruebas de referido: abre `t.me/<bot>?start=ref_<tu_id>` con otra cuenta y paga; el referidor recibe los días extra.
- [ ] Ningún usuario Free recibe precios antes de `FREE_SIGNAL_DELAY_HOURS`.
- [ ] Un respaldo de `data/` se puede restaurar.

## 6. Paso a producción

Pasa a producción solo si la marcha blanca dio resultados razonables (mira `/performance`, que distingue las señales en papel).

1. Deposita el capital. Empieza con 0,5% de riesgo por operación.
2. En `.env`:
   ```
   EXCHANGE_TYPE=hyperliquid_mainnet
   NETWORK=mainnet
   TREND_RISK_PER_TRADE=0.005
   ```
3. Reinicia con `sudo systemctl restart arbitrajebot`. En el log debe aparecer `Estrategia TENDENCIA (REAL)`.
4. En la primera entrada real, comprueba en app.hyperliquid.xyz que la posición tiene su **stop trigger** visible en *Open Orders*.
5. Empieza a vender Premium recién cuando tengas historial **real**.

Para actualizar el código: `./deploy/update.sh`. Respalda, compila, ejecuta los tests y reinicia.

Para volver atrás: `git checkout <commit anterior> && ./deploy/update.sh`. Si hace falta, restaura `backups/data_*.tar.gz` en `data/`.

## 7. Monetización: cómo funciona

| Elemento | Detalle |
|---|---|
| Free | Aviso inmediato "🔒 nueva señal" y señal completa tras `FREE_SIGNAL_DELAY_HOURS` |
| Premium | Señal al instante (entrada, stop, tamaño sugerido), stops movidos, cierres y `/signals` |
| Pago | Telegram Stars, `PREMIUM_PRICE_STARS` cada 30 días con renovación automática (el usuario la cancela desde Telegram) |
| Prueba | `/trial`: `TRIAL_DAYS` gratis, una vez por usuario |
| Referidos | El referidor gana `REFERRAL_BONUS_DAYS` cuando su invitado paga por primera vez |
| Retención | Aviso 3 días antes del vencimiento y otro al expirar |
| Admin | `/stats`, `/grant`, `/revoke`, `/refund`, `/broadcast_free`, `/broadcast_premium`, o el dashboard |

Las Stars se retiran desde Telegram vía Fragment, después del periodo de retención que fija Telegram.

## 8. Responsabilidad legal

- Cada señal y `/terms` incluyen el aviso de que no es asesoría financiera. No lo quites, y no prometas rentabilidad en la publicidad.
- `/performance` indica si incluye operaciones en papel. Publica resultados reales cuando los tengas.
- En Chile, la **Ley Fintec (21.521)** regula la asesoría de inversión. Antes de cobrar, consulta con un abogado si tu servicio requiere inscripción ante la CMF.
- `data/subscribers.json` contiene datos personales. No lo subas a git (`git rm --cached subscribers.json`) y respáldalo de forma privada.
