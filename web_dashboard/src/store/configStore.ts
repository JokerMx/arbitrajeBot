import { create } from "zustand";
import { persist } from "zustand/middleware";
import { 
  FullConfig, 
  StrategyConfig, 
  RiskConfig, 
  MarketConfig, 
  TelegramConfig,
  LeverageConfig,
  ProtectionConfig
} from "../services/types";
import { backendApiBaseUrl } from "../services/backendConfig";

const DEFAULT_CONFIG: FullConfig = {
  strategy: {
    baseSpreadTicks: 10,
    riskAversion: 0.15,
    marketImpact: 0.01,
    inventorySkewStrength: 0.15,
    volatilityLookbackMinutes: 20,
    orderRefreshMs: 30000,
    maxInventoryRatio: 0.10,
  },
  risk: {
    maxPositionSizePct: 0.10,
    maxDailyLossPct: 0.20,
    maxDrawdownPct: 0.15,
    killSwitchEnabled: true,
    circuitBreakerThreshold: 0.03,
  },
  markets: [
    { symbol: "BTC", enabled: true, marginAlloc: 0.10, minSize: 0.00001 },
    { symbol: "ETH", enabled: false, marginAlloc: 0.10, minSize: 0.0001 },
    { symbol: "BNB", enabled: false, marginAlloc: 0.10, minSize: 0.001 },
    { symbol: "SOL", enabled: false, marginAlloc: 0.10, minSize: 0.01 },
    { symbol: "XRP", enabled: false, marginAlloc: 0.10, minSize: 0.5 },
  ],
  telegram: {
    enabled: false,
    botToken: "",
    chatId: "",
    notifyOrders: true,
    notifyFills: true,
    notifyStatus: true,
  },
  initialCapital: 50,
  leverage: [
    { exchange: "simulated", symbol: "BTC", leverage: 1, maxLeverage: 10 },
    { exchange: "simulated", symbol: "ETH", leverage: 1, maxLeverage: 10 },
    { exchange: "simulated", symbol: "BNB", leverage: 1, maxLeverage: 10 },
    { exchange: "simulated", symbol: "SOL", leverage: 1, maxLeverage: 10 },
    { exchange: "simulated", symbol: "XRP", leverage: 1, maxLeverage: 10 },
    { exchange: "hyperliquid_mainnet", symbol: "BTC", leverage: 1, maxLeverage: 10 },
    { exchange: "hyperliquid_mainnet", symbol: "ETH", leverage: 1, maxLeverage: 10 },
    { exchange: "hyperliquid_mainnet", symbol: "BNB", leverage: 1, maxLeverage: 10 },
    { exchange: "hyperliquid_mainnet", symbol: "SOL", leverage: 1, maxLeverage: 10 },
    { exchange: "hyperliquid_mainnet", symbol: "XRP", leverage: 1, maxLeverage: 10 },
    { exchange: "bybit", symbol: "BTC", leverage: 1, maxLeverage: 10 },
    { exchange: "bybit", symbol: "ETH", leverage: 1, maxLeverage: 10 },
    { exchange: "bybit", symbol: "BNB", leverage: 1, maxLeverage: 10 },
    { exchange: "bybit", symbol: "SOL", leverage: 1, maxLeverage: 10 },
    { exchange: "bybit", symbol: "XRP", leverage: 1, maxLeverage: 10 },
  ],
  protection: {
    maxCapitalAtRiskPct: 0.25,
    autoStopLossEnabled: true,
    autoStopLossPct: 0.15,
    maxLeverage: 10,
    emergencyWithdrawEnabled: false,
  },
};

interface ConfigStore {
  config: FullConfig;
  isLoading: boolean;
  error: string | null;
  
  updateStrategy: (strategy: Partial<StrategyConfig>) => Promise<void>;
  updateRisk: (risk: Partial<RiskConfig>) => Promise<void>;
  updateMarket: (symbol: string, updates: Partial<MarketConfig>) => Promise<void>;
  updateTelegram: (telegram: Partial<TelegramConfig>) => Promise<void>;
  updateLeverage: (leverage: LeverageConfig[]) => Promise<void>;
  updateProtection: (protection: Partial<ProtectionConfig>) => Promise<void>;
  loadConfig: () => Promise<void>;
  saveConfig: () => Promise<void>;
  resetConfig: () => void;
}

export const useConfigStore = create<ConfigStore>()(
  persist(
    (set, get) => ({
      config: DEFAULT_CONFIG,
      isLoading: false,
      error: null,

      updateStrategy: async (strategy) => {
        set((state) => ({
          config: {
            ...state.config,
            strategy: { ...state.config.strategy, ...strategy },
          },
        }));
        await get().saveConfig();
      },

      updateRisk: async (risk) => {
        set((state) => ({
          config: {
            ...state.config,
            risk: { ...state.config.risk, ...risk },
          },
        }));
        await get().saveConfig();
      },

      updateMarket: async (symbol, updates) => {
        set((state) => ({
          config: {
            ...state.config,
            markets: state.config.markets.map((m) =>
              m.symbol === symbol ? { ...m, ...updates } : m
            ),
          },
        }));
        await get().saveConfig();
      },

      updateTelegram: async (telegram) => {
        set((state) => ({
          config: {
            ...state.config,
            telegram: { ...state.config.telegram, ...telegram },
          },
        }));
        await get().saveConfig();
      },

      updateLeverage: async (leverage) => {
        set((state) => ({
          config: {
            ...state.config,
            leverage,
          },
        }));
        await get().saveConfig();
      },

      updateProtection: async (protection) => {
        set((state) => ({
          config: {
            ...state.config,
            protection: { ...state.config.protection, ...protection },
          },
        }));
        await get().saveConfig();
      },

      loadConfig: async () => {
        set({ isLoading: true, error: null });
        try {
          const response = await fetch(`${backendApiBaseUrl}/config`);
          if (!response.ok) throw new Error("Error cargando configuración");
          const config = await response.json();
          set({ config, isLoading: false });
        } catch (error) {
          set({ error: (error as Error).message, isLoading: false });
        }
      },

      saveConfig: async () => {
        const { config } = get();
        try {
          const response = await fetch(`${backendApiBaseUrl}/config`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify(config),
          });
          if (!response.ok) throw new Error("Error guardando configuración");
        } catch (error) {
          set({ error: (error as Error).message });
        }
      },

      resetConfig: () => {
        set({ config: DEFAULT_CONFIG });
        get().saveConfig();
      },
    }),
    {
      name: "bot-config",
    }
  )
);