.
├── Cargo.lock
├── Cargo.toml
├── config.toml
├── crea_dashboard.sh
├── estructura.md
├── fix.sh
├── listado.txt
├── package-lock.json
├── run.sh
├── setup_dashboard.sh
├── src
│   ├── api
│   │   ├── handlers.rs
│   │   ├── mod.rs
│   │   ├── models.rs
│   │   └── server.rs
│   ├── bot.rs
│   ├── executor.rs
│   ├── lib.rs
│   ├── main.rs
│   ├── market.rs
│   ├── notifier.rs
│   ├── risk.rs
│   ├── strategy.rs
│   ├── telegram_bot.rs
│   └── types.rs
├── subscribers.json
└── web_dashboard
    ├── actualizar_dashboard.sh
    ├── index.html
    ├── package-lock.json
    ├── package.json
    ├── postcss.config.js
    ├── src
    │   ├── App.tsx
    │   ├── components
    │   │   ├── Admin
    │   │   │   ├── TelegramUsers.tsx
    │   │   │   └── UserDetailModal.tsx
    │   │   ├── Controls
    │   │   │   ├── MarketControls.tsx
    │   │   │   ├── RiskControls.tsx
    │   │   │   └── StrategyControls.tsx
    │   │   ├── Dashboard
    │   │   │   ├── BalanceChart.tsx
    │   │   │   ├── PositionsTable.tsx
    │   │   │   ├── StatsCards.tsx
    │   │   │   └── TradesTable.tsx
    │   │   └── Layout
    │   │       ├── DashboardLayout.tsx
    │   │       ├── Header.tsx
    │   │       └── Sidebar.tsx
    │   ├── index.css
    │   ├── main.tsx
    │   ├── services
    │   │   ├── telegramAdminApi.ts
    │   │   ├── types.ts
    │   │   └── websocket.ts
    │   ├── store
    │   │   ├── botStore.ts
    │   │   └── configStore.ts
    │   ├── types
    │   │   └── telegram.ts
    │   └── vite-env.d.ts
    ├── tailwind.config.js
    ├── tsconfig.json
    ├── tsconfig.node.json
    ├── verificar_dashboard.sh
    └── vite.config.ts

13 directories, 57 files
