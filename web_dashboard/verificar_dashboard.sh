#!/bin/bash

echo -e "${YELLOW}Verificando estructura final...${NC}"

# Array con todos los archivos que DEBEN existir
declare -a required_files=(
    "web_dashboard/index.html"
    "web_dashboard/package.json"
    "web_dashboard/tailwind.config.js"
    "web_dashboard/tsconfig.json"
    "web_dashboard/vite.config.ts"
    "web_dashboard/src/main.tsx"
    "web_dashboard/src/App.tsx"
    "web_dashboard/src/index.css"
    "web_dashboard/src/components/Layout/DashboardLayout.tsx"
    "web_dashboard/src/components/Layout/Header.tsx"
    "web_dashboard/src/components/Layout/Sidebar.tsx"
    "web_dashboard/src/components/Dashboard/StatsCards.tsx"
    "web_dashboard/src/components/Dashboard/BalanceChart.tsx"
    "web_dashboard/src/components/Dashboard/PositionsTable.tsx"
    "web_dashboard/src/components/Dashboard/TradesTable.tsx"
    "web_dashboard/src/components/Controls/StrategyControls.tsx"
    "web_dashboard/src/components/Controls/RiskControls.tsx"
    "web_dashboard/src/components/Controls/MarketControls.tsx"
    "web_dashboard/src/services/types.ts"
    "web_dashboard/src/services/websocket.ts"
    "web_dashboard/src/store/botStore.ts"
    "web_dashboard/src/store/configStore.ts"
)

# Array con archivos que NO deben existir
declare -a not_allowed_patterns=(
    "MarketOverview.tsx"
    "TelegramControls.tsx"
    "PnLChart.tsx"
    "WinRateChart.tsx"
    "DrawdownChart.tsx"
    "useWebSocket.ts"
    "useBotData.ts"
    "useConfig.ts"
    "api.ts"
    "formatters.ts"
    "constants.ts"
)

# Verificar archivos requeridos
echo -e "\n${YELLOW}Verificando archivos requeridos:${NC}"
missing_files=0
for file in "${required_files[@]}"; do
    if [ -f "$file" ]; then
        echo -e "${GREEN}  ✅ $file${NC}"
    else
        echo -e "${RED}  ❌ $file${NC}"
        missing_files=$((missing_files + 1))
    fi
done

# Verificar que no existan archivos no permitidos
echo -e "\n${YELLOW}Verificando archivos no permitidos:${NC}"
found_not_allowed=0
for pattern in "${not_allowed_patterns[@]}"; do
    found_files=$(find web_dashboard -name "$pattern" 2>/dev/null)
    if [ -n "$found_files" ]; then
        echo -e "${RED}  ❌ Encontrado no permitido: $pattern${NC}"
        found_not_allowed=$((found_not_allowed + 1))
    else
        echo -e "${GREEN}  ✅ No existe: $pattern${NC}"
    fi
done

# Resumen
echo -e "\n${YELLOW}=== RESUMEN ===${NC}"
if [ $missing_files -eq 0 ] && [ $found_not_allowed -eq 0 ]; then
    echo -e "${GREEN}✅ Estructura VERIFICADA: Correcta${NC}"
elif [ $missing_files -gt 0 ] && [ $found_not_allowed -eq 0 ]; then
    echo -e "${YELLOW}⚠️  Estructura PARCIAL: Faltan $missing_files archivos requeridos${NC}"
elif [ $missing_files -eq 0 ] && [ $found_not_allowed -gt 0 ]; then
    echo -e "${RED}❌ Estructura INCORRECTA: Hay $found_not_allowed archivos no permitidos${NC}"
else
    echo -e "${RED}❌ Estructura INCORRECTA: Faltan $missing_files archivos y hay $found_not_allowed no permitidos${NC}"
fi