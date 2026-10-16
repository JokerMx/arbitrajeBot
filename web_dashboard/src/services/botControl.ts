import React from "react";
import toast from "react-hot-toast";
import { backendApiBaseUrl } from "./backendConfig";

export interface BotStatusInfo {
  strategy_mode: "trend" | "market_making";
  live: boolean;
  rebound_live: boolean;
  paused: boolean;
  can_pause: boolean;
  uptime_secs: number;
  version: string;
  risk: { status: string; kill_switch: boolean; drawdown_pct: number | null };
}

/** Estado del bot, refrescado periódicamente */
export function useBotStatus(intervalMs = 15000) {
  const [status, setStatus] = React.useState<BotStatusInfo | null>(null);
  const [online, setOnline] = React.useState(true);

  const load = React.useCallback(async () => {
    try {
      const r = await fetch(`${backendApiBaseUrl}/bot/status`);
      if (!r.ok) throw new Error();
      setStatus(await r.json());
      setOnline(true);
    } catch {
      setOnline(false);
    }
  }, []);

  React.useEffect(() => {
    load();
    const id = setInterval(load, intervalMs);
    return () => clearInterval(id);
  }, [load, intervalMs]);

  return { status, online, reload: load };
}

async function post(path: string) {
  const r = await fetch(`${backendApiBaseUrl}${path}`, { method: "POST" });
  return r.json();
}

export async function pauseBot() {
  const res = await post("/bot/stop");
  res.success ? toast.success(res.message) : toast.error(res.message);
  return res;
}

export async function resumeBot() {
  const res = await post("/bot/start");
  res.success ? toast.success(res.message) : toast.error(res.message);
  return res;
}

export async function restartBot() {
  const res = await post("/bot/restart");
  if (res.success) toast.success(res.message, { duration: 8000 });
  else toast.error(res.message);
  return res;
}

export const formatUptime = (secs: number) => {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return d > 0 ? `${d} d ${h} h` : h > 0 ? `${h} h ${m} min` : `${m} min`;
};
