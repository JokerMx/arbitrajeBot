#!/bin/bash

# run.sh - Script para ejecutar el bot con logs en tiempo real

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Hyperliquid Market Making Bot${NC}"
echo -e "${GREEN}========================================${NC}"

mkdir -p logs
DATE=$(date +"%Y%m%d_%H%M%S")
LOG_FILE="logs/bot_$DATE.log"

echo -e "${YELLOW}📝 Logs guardados en: $LOG_FILE${NC}"

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

NETWORK=${NETWORK:-testnet}
echo -e "${CYAN}🌐 Red: $NETWORK${NC}"

echo -e "${CYAN}🔨 Compilando bot...${NC}"
cargo build --release

echo -e "${GREEN}🚀 Iniciando bot...${NC}"
echo -e "${YELLOW}📝 Logs también guardados en: $LOG_FILE${NC}"

RUST_LOG=${LOG_LEVEL:-info} stdbuf -oL -eL ./target/release/hyperliquid-mmaker-bot 2>&1 | tee -a "$LOG_FILE"

EXIT_CODE=${PIPESTATUS[0]}

if [ $EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}✅ Bot finalizado correctamente${NC}"
else
    echo -e "${RED}❌ Bot finalizado con error (código: $EXIT_CODE)${NC}"
    echo -e "${YELLOW}📋 Últimas líneas del log:${NC}"
    tail -5 "$LOG_FILE"
fi

exit $EXIT_CODE