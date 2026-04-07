import React from "react";
import { useBotStore } from "../../store/botStore";
import {
  DollarSign,
  TrendingUp,
  TrendingDown,
  Activity,
  Percent,
} from "lucide-react";

interface StatCardProps {
  title: string;
  value: string;
  change?: number;
  icon: React.ElementType;
  color: string;
  subtext?: string;
}

const StatCard: React.FC<StatCardProps> = ({
  title,
  value,
  change,
  icon: Icon,
  color,
  subtext,
}) => (
  <div className="bg-dark-200 rounded-xl p-6 border border-dark-300 hover:border-dark-400 transition-all">
    <div className="flex items-start justify-between">
      <div className="flex-1">
        <p className="text-gray-400 text-sm mb-1">{title}</p>
        <h3 className="text-2xl font-bold font-mono">{value}</h3>
        {change !== undefined && (
          <p
            className={`text-sm mt-2 flex items-center gap-1 ${
              change >= 0 ? "text-success" : "text-danger"
            }`}
          >
            {change >= 0 ? (
              <TrendingUp className="w-4 h-4" />
            ) : (
              <TrendingDown className="w-4 h-4" />
            )}
            {Math.abs(change).toFixed(2)}%
          </p>
        )}
        {subtext && <p className="text-xs text-gray-500 mt-2">{subtext}</p>}
      </div>
      <div className={`p-3 rounded-lg bg-${color}/10 text-${color}`}>
        <Icon className="w-6 h-6" />
      </div>
    </div>
  </div>
);

export const StatsCards: React.FC = () => {
  const { stats } = useBotStore();

  if (!stats) {
    return (
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {[...Array(4)].map((_, i) => (
          <div
            key={i}
            className="bg-dark-200 rounded-xl p-6 border border-dark-300 animate-pulse"
          >
            <div className="h-4 bg-dark-300 rounded w-1/2 mb-4"></div>
            <div className="h-8 bg-dark-300 rounded w-3/4"></div>
          </div>
        ))}
      </div>
    );
  }

  // Valores seguros con fallback a 0
  const balance = stats.balance ?? 0;
  const pnl = stats.pnl ?? 0;
  const pnlPercent = stats.pnlPercent ?? 0;
  const winRate = stats.winRate ?? 0;
  const drawdown = stats.drawdown ?? 0;
  const winningTrades = stats.winningTrades ?? 0;
  const losingTrades = stats.losingTrades ?? 0;
  const totalTrades = winningTrades + losingTrades;

  const getWinRateColor = (rate: number): string => {
    if (rate >= 60) return "success";
    if (rate >= 50) return "warning";
    return "danger";
  };

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
      <StatCard
        title="Balance"
        value={`$${balance.toFixed(2)}`}
        change={pnlPercent}
        icon={DollarSign}
        color="primary"
        subtext={`Inicial: $10.00`}
      />
      
      <StatCard
        title="P&L Total"
        value={`$${pnl.toFixed(2)}`}
        icon={TrendingUp}
        color={pnl >= 0 ? "success" : "danger"}
        subtext={pnl >= 0 ? "Ganancia" : "Pérdida"}
      />
      
      <StatCard
        title="Win Rate"
        value={`${winRate.toFixed(1)}%`}
        icon={Percent}
        color={getWinRateColor(winRate)}
        subtext={`${winningTrades}G / ${losingTrades}P (${totalTrades} total)`}
      />
      
      <StatCard
        title="Drawdown"
        value={`${drawdown.toFixed(2)}%`}
        icon={Activity}
        color={drawdown < 5 ? "success" : drawdown < 10 ? "warning" : "danger"}
        subtext={`Máx: ${drawdown.toFixed(1)}%`}
      />
    </div>
  );
};