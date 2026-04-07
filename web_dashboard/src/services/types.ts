// Tipos principales
export interface BotStatus {
  running: boolean;
  mode: "paper" | "live";
  connected: boolean;
  uptime: number;
}

export interface Position {
  symbol: string;
  size: number;
  entryPrice: number;
  currentPrice: number;
  unrealizedPnl: number;
  realizedPnl: number;
  timestamp: string;
}

export interface Trade {
  id: string;
  symbol: string;
  side: "COMPRA" | "VENTA";
  size: number;
  price: number;
  value: number;
  pnl: number;
  timestamp: string;
}

export interface BotStats {
  balance: number;
  pnl: number;
  pnlPercent: number;
  dailyPnl: number;
  drawdown: number;
  winRate: number;
  totalTrades: number;
  winningTrades: number;
  losingTrades: number;
  volume: number;
  positions: Position[];
  trades: Trade[];
  history: {
    timestamp: string;
    balance: number;
    pnl: number;
  }[];
}

export interface StrategyConfig {
  baseSpreadTicks: number;
  riskAversion: number;
  marketImpact: number;
  inventorySkewStrength: number;
  volatilityLookbackMinutes: number;
  orderRefreshMs: number;
  maxInventoryRatio: number;
}

export interface RiskConfig {
  maxPositionSizePct: number;
  maxDailyLossPct: number;
  maxDrawdownPct: number;
  killSwitchEnabled: boolean;
  circuitBreakerThreshold: number;
}

export interface MarketConfig {
  symbol: string;
  enabled: boolean;
  marginAlloc: number;
  minSize: number;
}

export interface TelegramConfig {
  enabled: boolean;
  botToken: string;
  chatId: string;
  notifyOrders: boolean;
  notifyFills: boolean;
  notifyStatus: boolean;
}

export interface FullConfig {
  strategy: StrategyConfig;
  risk: RiskConfig;
  markets: MarketConfig[];
  telegram: TelegramConfig;
  initialCapital: number;
}

export interface WebSocketMessage {
  type: "stats" | "trade" | "config" | "alert" | "heartbeat";
  data: any;
  timestamp: string;
}