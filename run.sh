#!/bin/bash

# run.sh - Script solo para Linux que ejecuta bot (Rust) y dashboard (Vite)

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

BOT_PID=""
DASH_PID=""

if [ "$(uname -s)" != "Linux" ]; then
    echo -e "${RED}❌ Este script está soportado únicamente en Linux${NC}"
    exit 1
fi

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Hyperliquid Market Making Bot${NC}"
echo -e "${GREEN}========================================${NC}"

mkdir -p logs
DATE=$(date +"%Y%m%d_%H%M%S")
BOT_LOG_FILE="logs/bot_$DATE.log"
DASH_LOG_FILE="logs/dashboard_$DATE.log"

echo -e "${YELLOW}📝 Log bot: $BOT_LOG_FILE${NC}"
echo -e "${YELLOW}📝 Log dashboard: $DASH_LOG_FILE${NC}"

cleanup() {
    echo -e "\n${YELLOW}🛑 Cerrando procesos...${NC}"

    if [ -n "$DASH_PID" ] && kill -0 "$DASH_PID" 2>/dev/null; then
        kill "$DASH_PID" 2>/dev/null || true
    fi

    if [ -n "$BOT_PID" ] && kill -0 "$BOT_PID" 2>/dev/null; then
        kill "$BOT_PID" 2>/dev/null || true
    fi
}

trap cleanup INT TERM EXIT

if [ -f .env ]; then
    echo -e "${CYAN}📄 Cargando configuración desde .env${NC}"
    set -a
    source <(grep -v '^#' .env | grep -v '^[[:space:]]*$' | sed 's/\r//')
    set +a
else
    echo -e "${RED}❌ Archivo .env no encontrado${NC}"
    exit 1
fi

if [ -z "$PRIVATE_KEY" ]; then
    echo -e "${RED}❌ PRIVATE_KEY no configurada${NC}"
    exit 1
fi

if [ -z "$NETWORK" ]; then
    echo -e "${RED}❌ NETWORK no está definido en .env${NC}"
    exit 1
fi
API_PORT=${API_PORT:-3001}
DASHBOARD_HOST=${DASHBOARD_HOST:-0.0.0.0}
DASHBOARD_PORT=${DASHBOARD_PORT:-5173}
DASHBOARD_MODE=${DASHBOARD_MODE:-embedded}
VITE_BACKEND_PROTOCOL=${VITE_BACKEND_PROTOCOL:-http}
VITE_BACKEND_HOST=${VITE_BACKEND_HOST:-localhost}
VITE_BACKEND_PORT=${VITE_BACKEND_PORT:-$API_PORT}

export VITE_BACKEND_PROTOCOL
export VITE_BACKEND_HOST
export VITE_BACKEND_PORT

echo -e "${CYAN}🌐 Red: $NETWORK${NC}"
echo -e "${CYAN}🌐 API: http://127.0.0.1:${API_PORT}${NC}"
echo -e "${CYAN}🌐 Dashboard mode: ${DASHBOARD_MODE}${NC}"
echo -e "${CYAN}🌐 Dashboard dev: http://127.0.0.1:${DASHBOARD_PORT}${NC}"
echo -e "${CYAN}🌐 Dashboard embebido: http://127.0.0.1:${API_PORT}/dashboard/${NC}"
echo -e "${CYAN}🌐 Frontend -> Backend: ${VITE_BACKEND_PROTOCOL}://${VITE_BACKEND_HOST}:${VITE_BACKEND_PORT}${NC}"

if [ ! -d "web_dashboard/node_modules" ]; then
    echo -e "${CYAN}📦 Instalando dependencias del dashboard...${NC}"
    (cd web_dashboard && npm install)
fi

if [ "$DASHBOARD_MODE" = "embedded" ]; then
    echo -e "${CYAN}🏗️  Compilando dashboard embebido...${NC}"
    (cd web_dashboard && npm run build)
elif [ "$DASHBOARD_MODE" != "dev" ]; then
    echo -e "${RED}❌ DASHBOARD_MODE inválido: $DASHBOARD_MODE${NC}"
    echo -e "${YELLOW}Usa DASHBOARD_MODE=embedded o DASHBOARD_MODE=dev${NC}"
    exit 1
fi

BINARY_PATH="./target/release/hyperliquid-mmaker-bot"

check_and_build() {
    if [ -f "$BINARY_PATH" ]; then
        local binary_time=$(stat -c %Y "$BINARY_PATH" 2>/dev/null)
        local src_time=0
        
        for src in src/*.rs; do
            if [ -f "$src" ]; then
                local src_file_time=$(stat -c %Y "$src" 2>/dev/null)
                if [ "$src_file_time" -gt "$src_time" ]; then
                    src_time=$src_file_time
                fi
            fi
        done
        
        if [ "$binary_time" -ge "$src_time" ]; then
            echo -e "${CYAN}⏭️  Binario ya compilado: $BINARY_PATH${NC}"
            return 1
        fi
        echo -e "${YELLOW}📜 Código fuente modificado, recompilando...${NC}"
    fi
    echo -e "${CYAN}🔨 Compilando bot...${NC}"
    cargo build --release
    return 0
}

if check_and_build; then
    echo -e "${GREEN}✅ Compilación completada${NC}"
fi

echo -e "${GREEN}🚀 Iniciando bot en segundo plano...${NC}"
RUST_LOG=${LOG_LEVEL:-info} PORT=${API_PORT} stdbuf -oL -eL ./target/release/hyperliquid-mmaker-bot \
    2>&1 | tee -a "$BOT_LOG_FILE" &
BOT_PID=$!

if [ "$DASHBOARD_MODE" = "dev" ]; then
    echo -e "${GREEN}🖥️  Iniciando dashboard (Vite) en segundo plano...${NC}"
    (cd web_dashboard && stdbuf -oL -eL npm run dev -- --host "$DASHBOARD_HOST" --port "$DASHBOARD_PORT" --strictPort) \
        2>&1 | tee -a "$DASH_LOG_FILE" &
    DASH_PID=$!
else
    echo -e "${GREEN}📦 Dashboard embebido habilitado; se servirá desde el backend Rust${NC}"
fi

echo -e "${GREEN}✅ Servicios iniciados${NC}"
echo -e "${YELLOW}   Bot PID: $BOT_PID${NC}"
if [ -n "$DASH_PID" ]; then
    echo -e "${YELLOW}   Dashboard PID: $DASH_PID${NC}"
fi
echo -e "${YELLOW}   Presiona Ctrl+C para detener ambos${NC}"

# Mantener script vivo y detectar salida inesperada de cualquier proceso
if [ -n "$DASH_PID" ]; then
    wait -n "$BOT_PID" "$DASH_PID"
else
    wait "$BOT_PID"
fi
EXIT_CODE=$?

if ! kill -0 "$BOT_PID" 2>/dev/null; then
    echo -e "${RED}❌ El bot se detuvo inesperadamente${NC}"
fi

if [ -n "$DASH_PID" ] && ! kill -0 "$DASH_PID" 2>/dev/null; then
    echo -e "${RED}❌ El dashboard se detuvo inesperadamente${NC}"
fi

if [ $EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✅ Finalizado correctamente${NC}"
else
    echo -e "${RED}❌ Finalizado con error (código: $EXIT_CODE)${NC}"
    echo -e "${YELLOW}📋 Últimas líneas del log del bot:${NC}"
    tail -5 "$BOT_LOG_FILE" || true
    if [ -n "$DASH_PID" ]; then
        echo -e "${YELLOW}📋 Últimas líneas del log del dashboard:${NC}"
        tail -5 "$DASH_LOG_FILE" || true
    fi
fi

exit $EXIT_CODE