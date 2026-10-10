#!/bin/bash

# Colores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Actualizando estructura de web_dashboard...${NC}"

# Crear estructura de carpetas (solo las necesarias)
mkdir -p web_dashboard/src/{components/{Layout,Dashboard,Controls},services,store}

# Crear archivos en la raíz (nueva estructura)
touch web_dashboard/index.html
touch web_dashboard/package.json
touch web_dashboard/tailwind.config.js
touch web_dashboard/tsconfig.json
touch web_dashboard/vite.config.ts

# Crear archivos en src/
touch web_dashboard/src/main.tsx
touch web_dashboard/src/App.tsx
touch web_dashboard/src/index.css

# Crear archivos en components/Layout
touch web_dashboard/src/components/Layout/DashboardLayout.tsx
touch web_dashboard/src/components/Layout/Header.tsx
touch web_dashboard/src/components/Layout/Sidebar.tsx

# Crear archivos en components/Dashboard
touch web_dashboard/src/components/Dashboard/StatsCards.tsx
touch web_dashboard/src/components/Dashboard/BalanceChart.tsx
touch web_dashboard/src/components/Dashboard/PositionsTable.tsx
touch web_dashboard/src/components/Dashboard/TradesTable.tsx

# Crear archivos en components/Controls
touch web_dashboard/src/components/Controls/StrategyControls.tsx
touch web_dashboard/src/components/Controls/RiskControls.tsx
touch web_dashboard/src/components/Controls/MarketControls.tsx

# Crear archivos en services
touch web_dashboard/src/services/types.ts
touch web_dashboard/src/services/websocket.ts

# Crear archivos en store
touch web_dashboard/src/store/botStore.ts
touch web_dashboard/src/store/configStore.ts

echo -e "${GREEN}✅ Archivos nuevos creados exitosamente${NC}"

# Eliminar archivos y carpetas que no están en la nueva estructura
echo -e "${YELLOW}Eliminando archivos y carpetas que no están en la nueva estructura...${NC}"

# Eliminar componentes que ya no existen
rm -f web_dashboard/src/components/Dashboard/MarketOverview.tsx 2>/dev/null && echo -e "${RED}  Eliminado: MarketOverview.tsx${NC}"
rm -f web_dashboard/src/components/Controls/TelegramControls.tsx 2>/dev/null && echo -e "${RED}  Eliminado: TelegramControls.tsx${NC}"
rm -rf web_dashboard/src/components/Charts 2>/dev/null && echo -e "${RED}  Eliminada carpeta: Charts/${NC}"

# Eliminar hooks completos (ya no existen en nueva estructura)
rm -rf web_dashboard/src/hooks 2>/dev/null && echo -e "${RED}  Eliminada carpeta: hooks/${NC}"

# Eliminar archivos de servicios que ya no existen
rm -f web_dashboard/src/services/api.ts 2>/dev/null && echo -e "${RED}  Eliminado: api.ts${NC}"

# Eliminar utils completos (ya no existen en nueva estructura)
rm -rf web_dashboard/src/utils 2>/dev/null && echo -e "${RED}  Eliminada carpeta: utils/${NC}"

# Verificar y eliminar carpetas vacías
find web_dashboard/src -type d -empty -delete 2>/dev/null

echo -e "${GREEN}✅ Estructura actualizada exitosamente!${NC}"
echo -e "\n${YELLOW}Estructura final:${NC}"
ls -R web_dashboard