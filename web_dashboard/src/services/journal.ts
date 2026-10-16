import React from "react";
import { backendApiBaseUrl } from "./backendConfig";

// Diario de operaciones del bot (GET /api/journal), construido desde el historial de señales.

export interface ClosedTrade {
  id: number;
  symbol: string;
  strategy: "tendencia" | "rebote" | string;
  paper: boolean;
  entry: number;
  exit: number;
  opened_at: string;
  closed_at: string;
  hours: number;
  pnl_pct: number;
  r: number | null;
  notional: number | null;
  pnl_usd: number | null;
  reason: string;
}

export interface OpenTrade {
  symbol: string;
  strategy: string;
  paper: boolean;
  entry: number;
  stop: number;
  initial_stop: number;
  opened_at: string;
  notional: number | null;
  current?: number;
  unrealized_pct?: number;
  unrealized_usd?: number;
}

export interface JournalStats {
  closed: number;
  wins: number;
  losses: number;
  win_rate: number;
  total_pnl_usd: number;
  avg_win_usd: number;
  avg_loss_usd: number;
  avg_win_pct: number;
  avg_loss_pct: number;
  profit_factor: number | null;
  expectancy_r: number | null;
  total_r: number;
  best_pct: number;
  worst_pct: number;
  avg_hours: number;
  max_drawdown_pct: number;
}

export interface Journal {
  closed: ClosedTrade[];
  open: OpenTrade[];
  stats: JournalStats;
  by_strategy: Record<string, JournalStats>;
  equity_curve: { t: string; equity: number }[];
  /** Foto de equity cada 5 minutos (últimos 7 días) */
  equity_series: { t: string; equity: number }[];
  initial_capital: number;
  equity_now: number;
  updated_at: string;
}

/** Carga el diario y lo refresca periódicamente */
export function useJournal(intervalMs = 30000) {
  const [data, setData] = React.useState<Journal | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [loading, setLoading] = React.useState(true);

  const load = React.useCallback(async () => {
    try {
      const r = await fetch(`${backendApiBaseUrl}/journal`);
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      setData(await r.json());
      setError(null);
    } catch (e) {
      setError("No se pudo cargar el diario de operaciones");
    } finally {
      setLoading(false);
    }
  }, []);

  React.useEffect(() => {
    load();
    const id = setInterval(load, intervalMs);
    return () => clearInterval(id);
  }, [load, intervalMs]);

  return { data, error, loading, reload: load };
}

// ---------- formato ----------
export const usd = (n: number | null | undefined, sign = false) => {
  if (n === null || n === undefined || Number.isNaN(n)) return "—";
  const s = Math.abs(n).toLocaleString("es-CL", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  if (!sign) return `$${n < 0 ? "-" : ""}${s}`;
  return `${n > 0 ? "+" : n < 0 ? "-" : ""}$${s}`;
};

export const pct = (n: number | null | undefined, digits = 2) =>
  n === null || n === undefined || Number.isNaN(n) ? "—" : `${n > 0 ? "+" : ""}${n.toFixed(digits)}%`;

export const price = (n: number | null | undefined) => {
  if (n === null || n === undefined) return "—";
  const d = n >= 1000 ? 2 : n >= 1 ? 4 : 6;
  return `$${n.toLocaleString("es-CL", { minimumFractionDigits: d, maximumFractionDigits: d })}`;
};

export const duration = (hours: number) => {
  if (hours < 1) return `${Math.round(hours * 60)} min`;
  if (hours < 48) return `${hours.toFixed(hours < 10 ? 1 : 0)} h`;
  return `${(hours / 24).toFixed(1)} d`;
};

export const pnlColor = (n: number | null | undefined) =>
  n === null || n === undefined || n === 0 ? "text-gray-300" : n > 0 ? "text-success" : "text-danger";

export const STRATEGY_LABEL: Record<string, string> = {
  tendencia: "Tendencia",
  rebote: "Rebote",
};
