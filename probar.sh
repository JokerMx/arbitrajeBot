#!/bin/bash
# Entorno de prueba local en modo papel (Docker). Sin dinero real y sin tu .env real.
#
#   ./probar.sh          prepara y levanta todo, espera a que el bot esté listo
#   ./probar.sh logs     sigue el log del bot
#   ./probar.sh reset    borra BD/datos de prueba y vuelve a levantar
#                        (necesario tras cambiar TELEGRAM_BOT_TOKEN en .env.test)
#   ./probar.sh stop     detiene el entorno (conserva datos)
set -euo pipefail
cd "$(dirname "$0")"

COMPOSE="docker compose -f docker-compose.test.yml"
G='\033[0;32m'; Y='\033[1;33m'; R='\033[0;31m'; C='\033[0;36m'; N='\033[0m'

case "${1:-up}" in
    logs)  exec $COMPOSE logs -f bot ;;
    stop)  $COMPOSE down; exit 0 ;;
    reset) $COMPOSE down -v ;;
    up)    ;;
    *)     echo "Uso: $0 [up|logs|reset|stop]"; exit 1 ;;
esac

docker info >/dev/null 2>&1 || { echo -e "${R}❌ Docker no está corriendo. Abre Docker Desktop.${N}"; exit 1; }

# 1. Variables de prueba
if [ ! -f .env.test ]; then
    KEY="0x$(od -An -tx1 -N32 /dev/urandom | tr -d ' \n')"
    sed "s|^PRIVATE_KEY=.*|PRIVATE_KEY=$KEY|" .env.test.example > .env.test
    echo -e "${C}📝 Creado .env.test con una clave desechable${N}"
fi
if ! grep -qE '^TELEGRAM_BOT_TOKEN=.+' .env.test; then
    echo -e "${Y}ℹ️  Telegram desactivado: pon TELEGRAM_BOT_TOKEN (bot de PRUEBA de @BotFather)"
    echo -e "   y TELEGRAM_CHAT_ID en .env.test, y ejecuta: ./probar.sh reset${N}"
fi

# 2. Dashboard (solo si cambió)
DIST=web_dashboard/dist/index.html
if [ ! -f "$DIST" ] || [ -n "$(find web_dashboard/src web_dashboard/index.html -newer "$DIST" 2>/dev/null | head -1)" ]; then
    echo -e "${C}🖥️  Compilando dashboard...${N}"
    ( cd web_dashboard && { [ -d node_modules ] || npm install; } && npm run build )
fi

# 3. Levantar
echo -e "${C}🐳 Levantando BD + bot (la primera compilación puede tardar ~30 min)...${N}"
$COMPOSE up -d

# 4. Esperar a que el bot esté listo (máx. 45 min)
for i in $(seq 1 540); do
    LOG=$($COMPOSE logs bot 2>&1 || true)
    # here-strings: con pipefail, `echo | grep -q` falla por SIGPIPE en logs grandes
    if grep -q "Tendencia: equity" <<< "$LOG"; then break; fi
    if grep -qE "could not compile|panicked" <<< "$LOG"; then
        echo -e "${R}❌ El bot falló. Últimas líneas:${N}"; $COMPOSE logs --tail 30 bot; exit 1
    fi
    RUNNING=$($COMPOSE ps --status running --services 2>/dev/null || true)
    if ! grep -qx bot <<< "$RUNNING"; then
        echo -e "${R}❌ El contenedor del bot se detuvo:${N}"; $COMPOSE logs --tail 30 bot; exit 1
    fi
    [ $((i % 6)) -eq 0 ] && echo "   ... $(echo "$LOG" | tail -1 | sed 's/\x1b\[[0-9;]*m//g' | cut -c1-110)"
    sleep 5
done

# 5. Verificar
TOKEN=$(grep -E '^ADMIN_API_TOKEN=' .env.test | cut -d= -f2- | tr -d '\r')
NO_AUTH=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:3001/api/stats || true)
AUTH=$(curl -s -o /dev/null -w "%{http_code}" -H "Authorization: Bearer $TOKEN" http://localhost:3001/api/stats || true)
if [ "$NO_AUTH" = "401" ] && [ "$AUTH" = "200" ]; then
    echo -e "${G}✅ Bot listo (modo PAPEL)${N}"
else
    echo -e "${Y}⚠️  API respondió sin token=$NO_AUTH con token=$AUTH (esperado 401/200)${N}"
fi

echo
echo -e "   Dashboard:  ${C}http://localhost:3001/dashboard/${N}   (token: ${TOKEN})"
echo -e "   Logs:       ./probar.sh logs"
echo -e "   Detener:    ./probar.sh stop"
$COMPOSE logs bot 2>&1 | sed 's/\x1b\[[0-9;]*m//g' | grep -E "TENDENCIA|Tendencia:|Bot de Telegram|ENTRADA" | tail -4 | sed 's/^bot-1  | /   /' || true
