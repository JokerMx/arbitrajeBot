#!/bin/bash

# Crear estructura de carpetas
mkdir -p web_dashboard/src/{components/{Layout,Dashboard,Controls,Charts},hooks,services,store,utils}

# Crear archivos en la raíz
touch web_dashboard/package.json
touch web_dashboard/tailwind.config.js
touch web_dashboard/tsconfig.json
touch web_dashboard/vite.config.ts
touch web_dashboard/index.html

# Crear archivos en src/
touch web_dashboard/src/main.tsx
touch web_dashboard/src/App.tsx
touch web_dashboard/src/index.css

# Crear archivos en components/Layout
touch web_dashboard/src/components/Layout/{Sidebar.tsx,Header.tsx,DashboardLayout.tsx}

# Crear archivos en components/Dashboard
touch web_dashboard/src/components/Dashboard/{StatsCards.tsx,BalanceChart.tsx,PositionsTable.tsx,TradesTable.tsx,MarketOverview.tsx}

# Crear archivos en components/Controls
touch web_dashboard/src/components/Controls/{StrategyControls.tsx,RiskControls.tsx,MarketControls.tsx,TelegramControls.tsx}

# Crear archivos en components/Charts
touch web_dashboard/src/components/Charts/{PnLChart.tsx,WinRateChart.tsx,DrawdownChart.tsx}

# Crear archivos en hooks
touch web_dashboard/src/hooks/{useWebSocket.ts,useBotData.ts,useConfig.ts}

# Crear archivos en services
touch web_dashboard/src/services/{api.ts,websocket.ts,types.ts}

# Crear archivos en store
touch web_dashboard/src/store/{botStore.ts,configStore.ts}

# Crear archivos en utils
touch web_dashboard/src/utils/{formatters.ts,constants.ts}

echo "✅ Estructura de web_dashboard creada exitosamente!"
ls -R web_dashboard