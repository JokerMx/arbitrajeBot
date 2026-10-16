import React, { useEffect, useState, useCallback } from "react";
import { Toaster } from "react-hot-toast";
import { DashboardLayout } from "./components/Layout/DashboardLayout";
import { StatsCards } from "./components/Dashboard/StatsCards";
import { BalanceChart } from "./components/Dashboard/BalanceChart";
import { PositionsTable } from "./components/Dashboard/PositionsTable";
import { TradesTable } from "./components/Dashboard/TradesTable";
import { JournalDashboard, JournalStatsView } from "./components/Dashboard/Journal";
import { StrategyControls } from "./components/Controls/StrategyControls";
import { RiskControls } from "./components/Controls/RiskControls";
import { MarketControls } from "./components/Controls/MarketControls";
import { TrendSymbols } from "./components/Controls/TrendSymbols";
import { SystemSettings } from "./components/Controls/SystemSettings";
import { TelegramSettings } from "./components/Controls/TelegramSettings";
import { LeverageControls } from "./components/Controls/LeverageControls";
import { ProtectionControls } from "./components/Controls/ProtectionControls";
import { TelegramUsers } from "./components/Admin/TelegramUsers";
import { useBotStore } from "./store/botStore";
import { useConfigStore } from "./store/configStore";
import { wsService } from "./services/websocket";
import { backendWsUrl } from "./services/backendConfig";

interface FillData {
  symbol: string;
  side: string;
  quantity: number;
  price: number;
  pnl: number;
  timestamp: string;
  id: string;
}

// =============================================
// COMPONENTES DE VISTAS
// =============================================

// Página principal: diario de operaciones construido desde el historial de señales
// (los "fills" del market making ya no aplican a la estrategia de tendencia/rebote)
const DashboardView: React.FC<{ fills: FillData[]; winningFills: number; losingFills: number; totalFillsValue: number }> = () => {
  return <JournalDashboard />;
};

// Estadísticas detalladas desde el diario de operaciones (antes: risk manager)
const StatsView: React.FC = () => <JournalStatsView />;

const StrategyView: React.FC = () => {
  console.log("📊 Renderizando StrategyView");
  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Configuración de Estrategia</h2>
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
        <StrategyControls />
      </div>
    </div>
  );
};

const RiskView: React.FC = () => {
  console.log("📊 Renderizando RiskView");
  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Gestión de Riesgo y Protección</h2>
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <RiskControls />
        <ProtectionControls />
        <div className="bg-dark-200 rounded-xl p-6 border border-dark-300 lg:col-span-2">
          <LeverageControls />
        </div>
      </div>
    </div>
  );
};

const MarketsView: React.FC = () => {
  console.log("📊 Renderizando MarketsView");
  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Configuración de Mercados</h2>
      <TrendSymbols />
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
        <MarketControls />
      </div>
    </div>
  );
};

const HistoryView: React.FC = () => {
  console.log("📊 Renderizando HistoryView");
  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Historial de Operaciones</h2>
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
        <TradesTable />
      </div>
    </div>
  );
};

const AlertsView: React.FC = () => {
  console.log("📊 Renderizando AlertsView");
  const { alerts } = useBotStore();
  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Alertas</h2>
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
        {alerts.length === 0 ? (
          <p className="text-gray-400 text-center py-8">No hay alertas</p>
        ) : (
          <div className="space-y-2">
            {alerts.map((alert, i) => (
              <div
                key={i}
                className="p-3 bg-dark-300 rounded-lg text-sm border border-dark-400"
              >
                {alert}
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};

const UsersView: React.FC = () => {
  console.log("📊 Renderizando UsersView");
  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Usuarios de Telegram</h2>
      <TelegramUsers />
    </div>
  );
};

const SettingsView: React.FC = () => {
  console.log("📊 Renderizando SettingsView");
  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Configuración General</h2>
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <SystemSettings />
        <TelegramSettings />
        <StrategyControls />
        <RiskControls />
        <LeverageControls />
        <ProtectionControls />
        <div className="bg-dark-200 rounded-xl p-6 border border-dark-300 lg:col-span-2">
          <MarketControls />
        </div>
      </div>
    </div>
  );
};

// =============================================
// COMPONENTE PRINCIPAL
// =============================================

function App() {
  const { status } = useBotStore();
  const { loadConfig } = useConfigStore();
  const [fills, setFills] = useState<FillData[]>([]);
  const [currentView, setCurrentView] = useState<string>("dashboard");

  useEffect(() => {
    loadConfig();
  console.log("🔌 Iniciando conexión WebSocket...");
  wsService.connect();  // Ya no necesita URL

  wsService.subscribe("alert", (data: string) => {
    console.log("📥 Alerta recibida:", data);
    
    if (data.startsWith("📊 FILL:")) {
      const parts = data.replace("📊 FILL:", "").split("|");
      
      if (parts.length >= 7) {
        const [symbol, side, quantityStr, priceStr, pnlStr, timestamp, id] = parts;
        
        const newFill = {
          symbol,
          side,
          quantity: parseFloat(quantityStr) || 0,
          price: parseFloat(priceStr) || 0,
          pnl: parseFloat(pnlStr) || 0,
          timestamp: timestamp || new Date().toLocaleTimeString(),
          id: id || `fill_${Date.now()}`,
        };
        
        setFills(prev => {
          if (prev.some(f => f.id === newFill.id)) {
            return prev;
          }
          return [newFill, ...prev].slice(0, 7);
        });
      }
    }
  });

  return () => {
    console.log("🔌 Cerrando conexión WebSocket...");
    wsService.disconnect();
  };
}, []);

  const handleNavigate = (view: string) => {
    console.log("🔄 handleNavigate llamado con:", view);
    console.log("   Vista anterior:", currentView);
    console.log("   Nueva vista:", view);
    setCurrentView(view);
  };

  // Calcular estadísticas de fills
  const winningFills = fills.filter(f => f.pnl > 0).length;
  const losingFills = fills.filter(f => f.pnl < 0).length;
  const totalFillsValue = fills.reduce((acc, f) => acc + Math.abs(f.pnl), 0);

  // Renderizar vista actual
  const renderView = () => {
    console.log("📄 renderView - Vista actual:", currentView);
    
    switch (currentView) {
      case "dashboard":
        return <DashboardView 
          fills={fills} 
          winningFills={winningFills} 
          losingFills={losingFills} 
          totalFillsValue={totalFillsValue} 
        />;
      case "stats":
        return <StatsView />;
      case "strategy":
        return <StrategyView />;
      case "risk":
        return <RiskView />;
      case "markets":
        return <MarketsView />;
      case "history":
        return <HistoryView />;
      case "alerts":
        return <AlertsView />;
      case "users":
        return <UsersView />;
      case "settings":
        return <SettingsView />;
      default:
        console.log("❓ Vista no encontrada:", currentView);
        return <div>Vista no encontrada: {currentView}</div>;
    }
  };

  return (
    <DashboardLayout onNavigate={handleNavigate} currentView={currentView}>
      <Toaster
        position="top-right"
        toastOptions={{
          style: {
            background: "#1e293b",
            color: "#fff",
            border: "1px solid #334155",
          },
        }}
      />

      <div className="flex justify-between items-center mb-6">
        <h1 className="text-2xl font-bold">
          {currentView === "dashboard" && "Dashboard"}
          {currentView === "stats" && "Estadísticas Detalladas"}
          {currentView === "strategy" && "Estrategia"}
          {currentView === "risk" && "Gestión de Riesgo"}
          {currentView === "markets" && "Mercados"}
          {currentView === "history" && "Historial"}
          {currentView === "alerts" && "Alertas"}
          {currentView === "users" && "Usuarios de Telegram"}
          {currentView === "settings" && "Configuración"}
        </h1>
        <div className="flex items-center gap-4">
          {status?.connected ? (
            <span className="flex items-center gap-2 text-success text-sm bg-success/10 px-3 py-1 rounded-full">
              <span className="w-2 h-2 bg-success rounded-full animate-pulse"></span>
              Conectado
            </span>
          ) : (
            <span className="flex items-center gap-2 text-gray-400 text-sm bg-dark-200 px-3 py-1 rounded-full">
              <span className="w-2 h-2 bg-gray-400 rounded-full"></span>
              Desconectado
            </span>
          )}
        </div>
      </div>

      {renderView()}
    </DashboardLayout>
  );
}

export default App;