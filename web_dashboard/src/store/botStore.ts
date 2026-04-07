import { create } from "zustand";
import { BotStats, BotStatus, Trade } from "../services/types";

interface BotStore {
  // Estado
  stats: BotStats | null;
  status: BotStatus | null;
  alerts: string[];
  
  // Acciones
  setStats: (stats: BotStats) => void;
  setStatus: (status: BotStatus) => void;
  addTrade: (trade: Trade) => void;
  addAlert: (alert: string) => void;
  clearAlerts: () => void;
  reset: () => void;
}

export const useBotStore = create<BotStore>((set) => ({
  stats: null,
  status: null,
  alerts: [],
  
  setStats: (stats) => {
    // Asegurar que todos los campos tengan valores por defecto
    const enhancedStats = {
      balance: stats.balance || 0,
      pnl: stats.pnl || 0,
      pnlPercent: stats.pnlPercent || 0,
      dailyPnl: stats.dailyPnl || 0,
      drawdown: stats.drawdown || 0,
      winRate: stats.winRate || 0,
      totalTrades: stats.totalTrades || 0,
      winningTrades: stats.winningTrades || 0,
      losingTrades: stats.losingTrades || 0,
      volume: stats.volume || 0,
      positions: stats.positions || [],
      trades: stats.trades || [],
      history: stats.history || [],
    };
    
    console.log("📦 Store: stats guardados:", enhancedStats);
    console.log("   winRate:", enhancedStats.winRate);
    console.log("   winningTrades:", enhancedStats.winningTrades);
    console.log("   losingTrades:", enhancedStats.losingTrades);
    
    set({ stats: enhancedStats });
  },
  
  setStatus: (status) => set({ status }),
  
  addTrade: (trade) =>
    set((state) => ({
      stats: state.stats
        ? {
            ...state.stats,
            trades: [trade, ...state.stats.trades].slice(0, 100),
          }
        : null,
    })),
    
  addAlert: (alert) =>
    set((state) => ({
      alerts: [alert, ...state.alerts].slice(0, 50),
    })),
    
  clearAlerts: () => set({ alerts: [] }),
  
  reset: () =>
    set({
      stats: null,
      status: null,
      alerts: [],
    }),
}));