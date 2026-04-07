import React, { useEffect, useState, useCallback } from "react";
import { Toaster } from "react-hot-toast";
import { DashboardLayout } from "./components/Layout/DashboardLayout";
import { StatsCards } from "./components/Dashboard/StatsCards";
import { BalanceChart } from "./components/Dashboard/BalanceChart";
import { PositionsTable } from "./components/Dashboard/PositionsTable";
import { TradesTable } from "./components/Dashboard/TradesTable";
import { StrategyControls } from "./components/Controls/StrategyControls";
import { RiskControls } from "./components/Controls/RiskControls";
import { MarketControls } from "./components/Controls/MarketControls";
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

const DashboardView: React.FC<{ fills: FillData[]; winningFills: number; losingFills: number; totalFillsValue: number }> = ({ 
  fills, winningFills, losingFills, totalFillsValue 
}) => {
  console.log("📊 Renderizando DashboardView");
  
  const formatPnl = (pnl: number): string => {
    return pnl > 0 ? `+${pnl.toFixed(2)}` : pnl.toFixed(2);
  };

  const getPnlColorClass = (pnl: number): string => {
    if (pnl > 0) return "text-success";
    if (pnl < 0) return "text-danger";
    return "text-gray-400";
  };

  const getPnlBgClass = (pnl: number): string => {
    if (pnl > 0) return "bg-success/10 border-success/20";
    if (pnl < 0) return "bg-danger/10 border-danger/20";
    return "bg-dark-300/50 border-dark-400";
  };

  return (
    <>
      <StatsCards />
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 mt-6">
        <div className="lg:col-span-2">
          <BalanceChart />
        </div>
        <div className="lg:col-span-1">
          <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
            <div className="flex justify-between items-center mb-4">
              <h3 className="text-lg font-semibold">Fills Recientes</h3>
              <div className="flex items-center gap-2 text-xs">
                <span className="text-success">+{winningFills}</span>
                <span className="text-gray-400">/</span>
                <span className="text-danger">{losingFills}</span>
                <span className="text-gray-400 ml-2">
                  ${totalFillsValue.toFixed(2)}
                </span>
              </div>
            </div>
            
            {fills.length === 0 ? (
              <div className="text-center py-8 text-gray-400">
                No hay fills disponibles
              </div>
            ) : (
              <div className="space-y-2 max-h-[320px] overflow-y-auto pr-1 scrollbar-thin scrollbar-thumb-dark-400 scrollbar-track-dark-300">
                {fills.map((fill) => (
                  <div
                    key={fill.id}
                    className={`p-2 rounded-lg border ${getPnlBgClass(fill.pnl)} transition-all hover:scale-[1.01] text-sm`}
                  >
                    <div className="flex justify-between items-center">
                      <div className="flex items-center gap-1.5">
                        <span className="font-bold text-white text-sm">{fill.symbol}</span>
                        <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${
                          fill.side === "COMPRA" 
                            ? "bg-success/20 text-success" 
                            : "bg-danger/20 text-danger"
                        }`}>
                          {fill.side === "COMPRA" ? "C" : "V"}
                        </span>
                      </div>
                      <span className="text-[10px] text-gray-400">
                        {fill.timestamp}
                      </span>
                    </div>
                    
                    <div className="flex justify-between items-center mt-1">
                      <span className="text-xs text-gray-300">
                        {fill.quantity.toFixed(6)} <span className="text-gray-500">@</span> ${fill.price.toFixed(2)}
                      </span>
                      <span className={`text-xs font-mono font-bold ${getPnlColorClass(fill.pnl)}`}>
                        {formatPnl(fill.pnl)} $
                      </span>
                    </div>
                    
                    {Math.abs(fill.pnl) > 0.005 && (
                      <div className="mt-1.5 h-1 w-full bg-dark-400 rounded-full overflow-hidden">
                        <div 
                          className={`h-full ${fill.pnl > 0 ? "bg-success" : "bg-danger"}`}
                          style={{ 
                            width: `${Math.min(Math.abs(fill.pnl) * 20, 100)}%`,
                            opacity: 0.7
                          }}
                        />
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
            
            {fills.length > 0 && (
              <div className="mt-3 pt-2 border-t border-dark-300 flex justify-between text-xs text-gray-400">
                <span>Mostrando {fills.length}/7 recientes</span>
                <span className="text-success">+{winningFills}</span>
                <span className="text-danger">-{losingFills}</span>
              </div>
            )}
          </div>
        </div>
      </div>
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6 mt-6">
        <PositionsTable />
        <TradesTable />
      </div>
    </>
  );
};

const StatsView: React.FC = () => {
  console.log("📊 Renderizando StatsView");
  const { stats } = useBotStore();
  
  const winningTrades = stats?.winningTrades || 0;
  const losingTrades = stats?.losingTrades || 0;
  const totalTrades = winningTrades + losingTrades;
  const winRate = stats?.winRate || 0;
  const balance = stats?.balance || 0;
  const pnl = stats?.pnl || 0;
  const drawdown = stats?.drawdown || 0;

  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Estadísticas Detalladas</h2>
      
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
          <p className="text-gray-400 text-sm">Balance</p>
          <p className="text-2xl font-bold text-white">${balance.toFixed(2)}</p>
          <p className="text-xs text-success mt-2">+{((balance/10 - 1) * 100).toFixed(2)}%</p>
        </div>
        <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
          <p className="text-gray-400 text-sm">P&L Total</p>
          <p className={`text-2xl font-bold ${pnl >= 0 ? 'text-success' : 'text-danger'}`}>
            ${pnl.toFixed(2)}
          </p>
          <p className="text-xs text-gray-400 mt-2">Desde inicio</p>
        </div>
        <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
          <p className="text-gray-400 text-sm">Win Rate</p>
          <p className="text-2xl font-bold text-white">{winRate.toFixed(1)}%</p>
          <p className="text-xs text-gray-400 mt-2">{winningTrades}G / {losingTrades}P</p>
        </div>
        <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
          <p className="text-gray-400 text-sm">Drawdown</p>
          <p className={`text-2xl font-bold ${drawdown < 10 ? 'text-success' : 'text-danger'}`}>
            {drawdown.toFixed(2)}%
          </p>
          <p className="text-xs text-gray-400 mt-2">Máximo: 20%</p>
        </div>
      </div>

      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
        <h3 className="text-lg font-semibold mb-4">Evolución del Balance</h3>
        <div className="h-80">
          <BalanceChart />
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <div className="bg-dark-200 p-6 rounded-xl border border-dark-300">
          <h3 className="text-lg font-semibold mb-4">Distribución de Trades</h3>
          <div className="space-y-4">
            <div>
              <div className="flex justify-between text-sm mb-1">
                <span className="text-gray-400">Ganadores</span>
                <span className="text-success">{winningTrades} ({((winningTrades/totalTrades)*100 || 0).toFixed(1)}%)</span>
              </div>
              <div className="w-full h-2 bg-dark-400 rounded-full overflow-hidden">
                <div 
                  className="h-full bg-success rounded-full" 
                  style={{ width: `${(winningTrades/totalTrades)*100 || 0}%` }}
                />
              </div>
            </div>
            <div>
              <div className="flex justify-between text-sm mb-1">
                <span className="text-gray-400">Perdedores</span>
                <span className="text-danger">{losingTrades} ({((losingTrades/totalTrades)*100 || 0).toFixed(1)}%)</span>
              </div>
              <div className="w-full h-2 bg-dark-400 rounded-full overflow-hidden">
                <div 
                  className="h-full bg-danger rounded-full" 
                  style={{ width: `${(losingTrades/totalTrades)*100 || 0}%` }}
                />
              </div>
            </div>
          </div>
        </div>

        <div className="bg-dark-200 p-6 rounded-xl border border-dark-300">
          <h3 className="text-lg font-semibold mb-4">Rendimiento por Hora</h3>
          <div className="h-48 flex items-center justify-center text-gray-400 bg-dark-300/50 rounded-lg">
            Próximamente...
          </div>
        </div>
      </div>

      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
        <h3 className="text-lg font-semibold mb-4">Últimos Trades</h3>
        <TradesTable />
      </div>
    </div>
  );
};

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