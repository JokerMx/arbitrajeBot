#!/bin/bash

# run.sh - Script para ejecutar el bot y el dashboard en ventanas separadas

set -e

# Colores para output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Hyperliquid Market Making Bot${NC}"
echo -e "${GREEN}  Con Dashboard Web${NC}"
echo -e "${GREEN}========================================${NC}"

# ============================================
# DETECCIÓN DEL SISTEMA
# ============================================

# Detectar sistema operativo
OS="$(uname -s)"
case "$OS" in
    Linux*)     MACHINE=Linux;;
    Darwin*)    MACHINE=Mac;;
    CYGWIN*|MINGW*|MSYS*) MACHINE=Windows;;
    *)          MACHINE="UNKNOWN"
esac

echo -e "${CYAN}💻 Sistema detectado: $MACHINE${NC}"

# ============================================
# VERIFICAR TERMINAL DISPONIBLE
# ============================================

detect_terminal() {
    if [ "$MACHINE" = "Linux" ]; then
        if command -v gnome-terminal >/dev/null 2>&1; then
            echo "gnome-terminal"
        elif command -v xterm >/dev/null 2>&1; then
            echo "xterm"
        elif command -v konsole >/dev/null 2>&1; then
            echo "konsole"
        else
            echo "none"
        fi
    elif [ "$MACHINE" = "Mac" ]; then
        if command -v osascript >/dev/null 2>&1; then
            echo "apple-script"
        else
            echo "none"
        fi
    elif [ "$MACHINE" = "Windows" ]; then
        echo "windows"
    else
        echo "none"
    fi
}

TERMINAL=$(detect_terminal)
echo -e "${CYAN}🖥️  Terminal detectada: $TERMINAL${NC}"

# ============================================
# CARGAR VARIABLES DE ENTORNO
# ============================================

if [ -f .env ]; then
    echo -e "${CYAN}📄 Cargando configuración desde .env${NC}"
    set -a
    source <(grep -v '^#' .env | grep -v '^[[:space:]]*$' | sed 's/\r//')
    set +a
else
    echo -e "${RED}❌ Archivo .env no encontrado${NC}"
    exit 1
fi

# Verificar variables requeridas
if [ -z "$PRIVATE_KEY" ] || [ "$PRIVATE_KEY" = "your_ethereum_private_key_here" ]; then
    echo -e "${RED}❌ Error: PRIVATE_KEY no configurada en .env${NC}"
    exit 1
fi

# ============================================
# COMPILAR PROYECTOS
# ============================================

echo -e "${CYAN}🔨 Compilando bot Rust...${NC}"
cargo build --release

echo -e "${CYAN}🔨 Compilando dashboard React...${NC}"
cd web_dashboard
if [ ! -d "node_modules" ]; then
    echo -e "${YELLOW}📦 Instalando dependencias del dashboard...${NC}"
    npm install
fi
npm run build
cd ..

# ============================================
# FUNCIONES PARA ABRIR TERMINALES
# ============================================

# Crear directorio de logs
mkdir -p logs
DATE=$(date +"%Y%m%d_%H%M%S")
BOT_LOG="logs/bot_$DATE.log"
DASHBOARD_LOG="logs/dashboard_$DATE.log"

echo -e "${YELLOW}📝 Logs del bot: $BOT_LOG${NC}"
echo -e "${YELLOW}📝 Logs del dashboard: $DASHBOARD_LOG${NC}"

# Función para Linux
open_linux_terminals() {
    BOT_CMD="cd $(pwd) && RUST_LOG=${LOG_LEVEL:-info} ./target/release/hyperliquid-mmaker-bot 2>&1 | tee $BOT_LOG; echo '✅ Bot finalizado. Presiona Enter para cerrar...'; read"
    DASHBOARD_CMD="cd $(pwd)/web_dashboard && npm run dev 2>&1 | tee ../$DASHBOARD_LOG; echo '✅ Dashboard finalizado. Presiona Enter para cerrar...'; read"

    case "$TERMINAL" in
        gnome-terminal)
            gnome-terminal --tab --title="Bot Hyperliquid" -- bash -c "$BOT_CMD"
            gnome-terminal --tab --title="Dashboard React" -- bash -c "$DASHBOARD_CMD"
            ;;
        xterm)
            xterm -T "Bot Hyperliquid" -e bash -c "$BOT_CMD" &
            xterm -T "Dashboard React" -e bash -c "$DASHBOARD_CMD" &
            ;;
        konsole)
            konsole --new-tab -p tabtitle="Bot Hyperliquid" -e bash -c "$BOT_CMD" &
            konsole --new-tab -p tabtitle="Dashboard React" -e bash -c "$DASHBOARD_CMD" &
            ;;
        *)
            echo -e "${YELLOW}⚠️  No se detectó terminal gráfica. Ejecutando en esta terminal...${NC}"
            echo -e "${CYAN}📋 Para ver el dashboard, abre otra terminal y ejecuta: cd web_dashboard && npm run dev${NC}"
            RUST_LOG=${LOG_LEVEL:-info} ./target/release/hyperliquid-mmaker-bot 2>&1 | tee "$BOT_LOG"
            ;;
    esac
}

# Función para Mac
open_mac_terminals() {
    BOT_CMD="cd $(pwd) && RUST_LOG=${LOG_LEVEL:-info} ./target/release/hyperliquid-mmaker-bot 2>&1 | tee $BOT_LOG"
    DASHBOARD_CMD="cd $(pwd)/web_dashboard && npm run dev 2>&1 | tee ../$DASHBOARD_LOG"

    osascript <<EOF
tell application "Terminal"
    activate
    do script "$BOT_CMD"
    set custom title of front window to "Bot Hyperliquid"
    
    delay 1
    do script "$DASHBOARD_CMD"
    set custom title of front window to "Dashboard React"
end tell
EOF
}

# Función para Windows (Git Bash)
open_windows_terminals() {
    BOT_CMD="cd $(pwd) && RUST_LOG=${LOG_LEVEL:-info} ./target/release/hyperliquid-mmaker-bot.exe 2>&1 | tee $BOT_LOG; echo '✅ Bot finalizado. Presiona Enter para cerrar...'; read"
    DASHBOARD_CMD="cd $(pwd)/web_dashboard && npm run dev 2>&1 | tee ../$DASHBOARD_LOG; echo '✅ Dashboard finalizado. Presiona Enter para cerrar...'; read"

    # Usar start para abrir nuevas ventanas de cmd
    start "Bot Hyperliquid" cmd /k "$BOT_CMD"
    start "Dashboard React" cmd /k "$DASHBOARD_CMD"
}

# ============================================
# FUNCIÓN PRINCIPAL
# ============================================

main() {
    echo -e "${GREEN}🚀 Iniciando bot y dashboard...${NC}"
    
    case "$MACHINE" in
        Linux)
            if [ "$TERMINAL" != "none" ]; then
                open_linux_terminals
            else
                echo -e "${YELLOW}⚠️  No se detectó terminal gráfica. Modo consola única.${NC}"
                echo -e "${CYAN}📋 Para ver el dashboard, abre otra terminal y ejecuta: cd web_dashboard && npm run dev${NC}"
                RUST_LOG=${LOG_LEVEL:-info} ./target/release/hyperliquid-mmaker-bot 2>&1 | tee "$BOT_LOG"
            fi
            ;;
        Mac)
            open_mac_terminals
            ;;
        Windows)
            open_windows_terminals
            ;;
        *)
            echo -e "${RED}❌ Sistema no soportado para múltiples terminales${NC}"
            echo -e "${CYAN}Ejecutando bot en esta terminal...${NC}"
            RUST_LOG=${LOG_LEVEL:-info} ./target/release/hyperliquid-mmaker-bot 2>&1 | tee "$BOT_LOG"
            ;;
    esac
    
    echo -e "${GREEN}✅ Bot y dashboard iniciados${NC}"
    echo -e "${YELLOW}📝 Logs guardados en:${NC}"
    echo -e "   Bot: $BOT_LOG"
    echo -e "   Dashboard: $DASHBOARD_LOG"
    echo ""
    echo -e "${CYAN}🌐 Dashboard disponible en: http://localhost:3001/dashboard${NC}"
    echo -e "${CYAN}🌐 API disponible en: http://localhost:3001/api/stats${NC}"
    echo ""
    echo -e "${YELLOW}⚠️  Cierra las terminales manualmente cuando termines${NC}"
}

# ============================================
# EJECUTAR
# ============================================

main

# Mantener script corriendo si estamos en modo consola única
if [ "$MACHINE" = "Linux" ] && [ "$TERMINAL" = "none" ]; then
    echo ""
    echo -e "${YELLOW}Presiona Ctrl+C para detener el bot${NC}"
    wait
fi