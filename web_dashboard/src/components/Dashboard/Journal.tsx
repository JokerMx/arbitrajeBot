import React from "react";
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  ReferenceLine,
  ReferenceDot,
} from "recharts";
import { format, parseISO } from "date-fns";
import { es } from "date-fns/locale";
import {
  Wallet,
  TrendingUp,
  Target,
  Scale,
  Activity,
  TrendingDown,
  Loader2,
  AlertCircle,
  RefreshCw,
  Download,
} from "lucide-react";
import {
  useJournal,
  Journal,
  JournalStats,
  usd,
  pct,
  price,
  duration,
  pnlColor,
  STRATEGY_LABEL,
  downloadTradesCsv,
} from "../../services/journal";

const fmtDate = (iso: string, f = "dd/MM HH:mm") => {
  try {
    return format(parseISO(iso), f, { locale: es });
  } catch {
    return iso;
  }
};

const Card: React.FC<{ className?: string; children: React.ReactNode }> = ({ className = "", children }) => (
  <div className={`bg-dark-200 rounded-xl p-6 border border-dark-300 ${className}`}>{children}</div>
);

const StrategyBadge: React.FC<{ strategy: string; paper?: boolean }> = ({ strategy, paper }) => (
  <span className="inline-flex items-center gap-1">
    <span
      className={`text-[11px] px-2 py-0.5 rounded-full font-medium ${
        strategy === "rebote" ? "bg-info/15 text-info" : "bg-primary-500/15 text-primary-300"
      }`}
    >
      {strategy === "rebote" ? "🔄 " : "🚀 "}
      {STRATEGY_LABEL[strategy] ?? strategy}
    </span>
    {paper && <span className="text-[10px] px-1.5 py-0.5 rounded bg-dark-400 text-gray-400">papel</span>}
  </span>
);

// ---------------- Tarjetas ----------------
const Kpi: React.FC<{
  title: string;
  value: string;
  sub?: string;
  icon: React.ElementType;
  tone?: "success" | "danger" | "neutral" | "warning";
}> = ({ title, value, sub, icon: Icon, tone = "neutral" }) => {
  const color =
    tone === "success" ? "text-success" : tone === "danger" ? "text-danger" : tone === "warning" ? "text-warning" : "text-white";
  return (
    <Card className="!p-5">
      <div className="flex items-start justify-between">
        <div>
          <p className="text-gray-400 text-xs mb-1">{title}</p>
          <p className={`text-2xl font-bold font-mono ${color}`}>{value}</p>
          {sub && <p className="text-xs text-gray-500 mt-1">{sub}</p>}
        </div>
        <Icon className="w-5 h-5 text-gray-500" />
      </div>
    </Card>
  );
};

const KpiRow: React.FC<{ j: Journal }> = ({ j }) => {
  const s = j.stats;
  const unrealized = j.open.reduce((acc, o) => acc + (o.unrealized_usd ?? 0), 0);
  const totalPnl = s.total_pnl_usd + unrealized;
  const totalPct = j.initial_capital > 0 ? (totalPnl / j.initial_capital) * 100 : 0;
  const tone = (n: number) => (n > 0 ? "success" : n < 0 ? "danger" : "neutral");
  return (
    <div className="grid grid-cols-2 md:grid-cols-3 xl:grid-cols-6 gap-4">
      <Kpi
        title="Equity"
        value={usd(j.initial_capital + totalPnl)}
        sub={`inicial ${usd(j.initial_capital)}`}
        icon={Wallet}
      />
      <Kpi
        title="P&L total"
        value={usd(totalPnl, true)}
        sub={`${pct(totalPct)} · realizado ${usd(s.total_pnl_usd, true)}${
          j.open.length ? ` · abierto ${usd(unrealized, true)}` : ""
        }`}
        icon={TrendingUp}
        tone={tone(totalPnl)}
      />
      <Kpi
        title="Aciertos"
        value={s.closed ? `${(s.win_rate * 100).toFixed(0)}%` : "—"}
        sub={`${s.wins} ganadas · ${s.losses} perdidas`}
        icon={Target}
      />
      <Kpi
        title="Profit factor"
        value={s.profit_factor !== null ? s.profit_factor.toFixed(2) : s.wins > 0 ? "∞" : "—"}
        sub={`ganancia media ${usd(s.avg_win_usd, true)} · pérdida media ${usd(s.avg_loss_usd, true)}`}
        icon={Scale}
        tone={s.profit_factor !== null ? (s.profit_factor >= 1 ? "success" : "danger") : "neutral"}
      />
      <Kpi
        title="R medio por operación"
        value={s.expectancy_r !== null ? `${s.expectancy_r >= 0 ? "+" : ""}${s.expectancy_r.toFixed(2)}R` : "—"}
        sub={`total ${s.total_r >= 0 ? "+" : ""}${s.total_r.toFixed(2)}R · duración media ${s.closed ? duration(s.avg_hours) : "—"}`}
        icon={Activity}
        tone={s.expectancy_r !== null ? tone(s.expectancy_r) : "neutral"}
      />
      <Kpi
        title="Caída máxima"
        value={`${s.max_drawdown_pct.toFixed(2)}%`}
        sub={`mejor ${pct(s.closed ? s.best_pct : null)} · peor ${pct(s.closed ? s.worst_pct : null)}`}
        icon={TrendingDown}
        tone={s.max_drawdown_pct > 10 ? "danger" : s.max_drawdown_pct > 5 ? "warning" : "neutral"}
      />
    </div>
  );
};

// ---------------- Curva de equity ----------------
const RANGES = [
  { key: "24h", label: "24 h", hours: 24 },
  { key: "3d", label: "3 días", hours: 72 },
  { key: "7d", label: "7 días", hours: 168 },
  { key: "30d", label: "30 días", hours: 720 },
] as const;

export const EquityCurve: React.FC<{ j: Journal; height?: number }> = ({ j, height = 256 }) => {
  const [range, setRange] = React.useState<(typeof RANGES)[number]["key"]>("3d");
  // Escala ajustada a los datos (se aprecian movimientos de centavos) o desde el capital inicial
  const [fit, setFit] = React.useState(true);
  const hours = RANGES.find((r) => r.key === range)!.hours;
  const since = Date.now() - hours * 3600_000;

  // Fotos cada 5 min; si aún no hay suficientes, se usa la curva por operación
  const source = (j.equity_series?.length ?? 0) >= 2 ? j.equity_series : j.equity_curve;
  const unrealized = j.open.reduce((acc, o) => acc + (o.unrealized_usd ?? 0), 0);
  const realizedNow = j.initial_capital + j.stats.total_pnl_usd;
  const points = source
    .map((p) => ({ ts: parseISO(p.t).getTime(), equity: Number(p.equity.toFixed(2)) }))
    .filter((p) => p.ts >= since);
  // Último punto = ahora (incluye posiciones abiertas a precio actual)
  points.push({ ts: parseISO(j.updated_at).getTime(), equity: Number((realizedNow + unrealized).toFixed(2)) });

  // Marcas de cada operación cerrada en el rango
  const closes = j.closed
    .map((t) => ({ ts: parseISO(t.closed_at).getTime(), win: (t.pnl_usd ?? t.pnl_pct) >= 0, t }))
    .filter((c) => c.ts >= since);
  const equityAt = (ts: number) => {
    let e = points[0]?.equity ?? j.initial_capital;
    for (const p of points) {
      if (p.ts <= ts) e = p.equity;
      else break;
    }
    return e;
  };

  const values = points.map((p) => p.equity);
  const min = fit ? Math.min(...values) : Math.min(...values, j.initial_capital);
  const max = fit ? Math.max(...values) : Math.max(...values, j.initial_capital);
  // Margen: 15% del rango, con un mínimo pequeño para que una línea plana quede centrada
  const pad = Math.max((max - min) * 0.15, (fit ? 0.0002 : 0.002) * j.initial_capital);
  const last = points[points.length - 1];
  const first = points[0];
  const up = last.equity >= j.initial_capital;
  const change = last.equity - first.equity;
  const changePct = first.equity ? (change / first.equity) * 100 : 0;
  const yDecimals = max - min < 5 ? 2 : 0;
  const color = up ? "#22c55e" : "#ef4444";
  const tickFmt = (ts: number) => format(new Date(ts), hours <= 24 ? "HH:mm" : "dd/MM HH:mm", { locale: es });

  return (
    <Card>
      <div className="flex flex-wrap items-center justify-between gap-2 mb-4">
        <div>
          <h3 className="text-lg font-semibold">Evolución del balance</h3>
          <p className="text-xs text-gray-400">
            Foto cada 5 min (capital + ganancias cerradas + posiciones abiertas a precio actual) · ● cierre de operación
          </p>
          <p className={`text-sm font-mono mt-1 ${pnlColor(change)}`}>
            {usd(change, true)} ({pct(changePct)}) en el período
            {Math.abs(change) < 0.005 && <span className="text-gray-500 text-xs font-sans"> · sin cambios: no hubo posiciones abiertas</span>}
          </p>
        </div>
        <div className="flex flex-wrap gap-1 text-xs">
          <button
            onClick={() => setFit(!fit)}
            className="px-3 py-1 rounded-lg bg-dark-300 text-gray-300 hover:bg-dark-400 mr-2"
            title="Escala ajustada a los datos o desde el capital inicial"
          >
            Escala: {fit ? "ajustada" : "desde el inicio"}
          </button>
          {RANGES.map((r) => (
            <button
              key={r.key}
              onClick={() => setRange(r.key)}
              className={`px-3 py-1 rounded-lg ${
                range === r.key ? "bg-primary-600 text-white" : "bg-dark-300 text-gray-400 hover:bg-dark-400"
              }`}
            >
              {r.label}
            </button>
          ))}
        </div>
      </div>
      {points.length < 2 ? (
        <div style={{ height }} className="flex items-center justify-center text-gray-400 text-sm text-center">
          Recolectando datos: aparece un punto nuevo cada 5 minutos.
        </div>
      ) : (
        <div style={{ height }}>
          <ResponsiveContainer width="100%" height="100%">
            <AreaChart data={points} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
              <defs>
                <linearGradient id="eqFill" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor={color} stopOpacity={0.35} />
                  <stop offset="100%" stopColor={color} stopOpacity={0} />
                </linearGradient>
              </defs>
              <CartesianGrid strokeDasharray="3 3" stroke="#2a2f3a" />
              <XAxis
                dataKey="ts"
                type="number"
                scale="time"
                domain={["dataMin", "dataMax"]}
                tickFormatter={tickFmt}
                stroke="#6b7280"
                fontSize={11}
                minTickGap={40}
              />
              <YAxis
                stroke="#6b7280"
                fontSize={11}
                domain={[min - pad, max + pad]}
                tickFormatter={(v) => `$${Number(v).toFixed(yDecimals)}`}
                allowDataOverflow
                width={72}
              />
              <Tooltip
                contentStyle={{ background: "#151922", border: "1px solid #2a2f3a", borderRadius: 8 }}
                labelFormatter={(ts) => format(new Date(Number(ts)), "dd/MM/yyyy HH:mm", { locale: es })}
                formatter={(v: number) => [
                  `${usd(v)} (${pct(j.initial_capital ? (v / j.initial_capital - 1) * 100 : 0)})`,
                  "Equity",
                ]}
              />
              <ReferenceLine
                y={j.initial_capital}
                stroke="#6b7280"
                strokeDasharray="4 4"
                ifOverflow="discard"
                label={{ value: "capital inicial", position: "insideBottomLeft", fill: "#6b7280", fontSize: 10 }}
              />
              <Area type="monotone" dataKey="equity" stroke={color} strokeWidth={2} fill="url(#eqFill)" dot={false} isAnimationActive={false} />
              {closes.map((c) => (
                <ReferenceDot
                  key={c.t.id}
                  x={c.ts}
                  y={equityAt(c.ts)}
                  r={5}
                  fill={c.win ? "#22c55e" : "#ef4444"}
                  stroke="#0f1218"
                  strokeWidth={2}
                />
              ))}
            </AreaChart>
          </ResponsiveContainer>
        </div>
      )}
    </Card>
  );
};

// ---------------- Por estrategia ----------------
const StrategyBreakdown: React.FC<{ j: Journal }> = ({ j }) => (
  <Card>
    <h3 className="text-lg font-semibold mb-4">Por estrategia</h3>
    <div className="space-y-3">
      {Object.entries(j.by_strategy).map(([name, s]: [string, JournalStats]) => {
        const open = j.open.filter((o) => o.strategy === name).length;
        return (
          <div key={name} className="p-4 rounded-lg bg-dark-300/50 border border-dark-400">
            <div className="flex items-center justify-between mb-2">
              <StrategyBadge strategy={name} />
              <span className={`font-mono font-bold ${pnlColor(s.total_pnl_usd)}`}>{usd(s.total_pnl_usd, true)}</span>
            </div>
            <div className="grid grid-cols-3 gap-2 text-xs text-gray-400">
              <div>
                <p>Cerradas</p>
                <p className="text-white font-medium">
                  {s.closed}
                  {open ? <span className="text-gray-500"> (+{open} abiertas)</span> : null}
                </p>
              </div>
              <div>
                <p>Aciertos</p>
                <p className="text-white font-medium">{s.closed ? `${(s.win_rate * 100).toFixed(0)}%` : "—"}</p>
              </div>
              <div>
                <p>R medio</p>
                <p className={`font-medium ${pnlColor(s.expectancy_r)}`}>
                  {s.expectancy_r !== null ? `${s.expectancy_r >= 0 ? "+" : ""}${s.expectancy_r.toFixed(2)}R` : "—"}
                </p>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  </Card>
);

// ---------------- Posiciones abiertas ----------------
const OpenPositions: React.FC<{ j: Journal }> = ({ j }) => (
  <Card>
    <div className="flex items-center justify-between mb-4">
      <h3 className="text-lg font-semibold">Posiciones abiertas</h3>
      <span className="text-xs text-gray-400">{j.open.length} abiertas</span>
    </div>
    {j.open.length === 0 ? (
      <p className="text-sm text-gray-400 py-6 text-center">
        Sin posiciones abiertas. El bot evalúa cada hora en punto.
      </p>
    ) : (
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-gray-400 border-b border-dark-300">
              <th className="pb-2 pr-3">Moneda</th>
              <th className="pb-2 pr-3">Estrategia</th>
              <th className="pb-2 pr-3">Entrada</th>
              <th className="pb-2 pr-3">Actual</th>
              <th className="pb-2 pr-3">Stop</th>
              <th className="pb-2 pr-3">Tamaño</th>
              <th className="pb-2 pr-3">P&L</th>
              <th className="pb-2 pr-3">% cuenta</th>
              <th className="pb-2">Abierta</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-dark-300">
            {j.open.map((o) => (
              <tr key={o.symbol}>
                <td className="py-2 pr-3 font-semibold">{o.symbol}</td>
                <td className="py-2 pr-3">
                  <StrategyBadge strategy={o.strategy} paper={o.paper} />
                </td>
                <td className="py-2 pr-3 font-mono">{price(o.entry)}</td>
                <td className="py-2 pr-3 font-mono">{price(o.current)}</td>
                <td className="py-2 pr-3 font-mono text-gray-400">
                  {price(o.stop)}
                  {o.stop > o.initial_stop && <span className="ml-1 text-success text-xs">↑</span>}
                </td>
                <td className="py-2 pr-3 font-mono">{usd(o.notional)}</td>
                <td className={`py-2 pr-3 font-mono ${pnlColor(o.unrealized_usd)}`}>
                  {usd(o.unrealized_usd, true)} <span className="text-xs">({pct(o.unrealized_pct)})</span>
                </td>
                <td className={`py-2 pr-3 font-mono ${pnlColor(o.unrealized_usd)}`}>
                  {o.unrealized_usd !== undefined
                    ? pct((o.unrealized_usd / (j.initial_capital + j.stats.total_pnl_usd)) * 100)
                    : "—"}
                </td>
                <td className="py-2 text-gray-400">{fmtDate(o.opened_at)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    )}
  </Card>
);

// ---------------- Operaciones cerradas ----------------
export const ClosedTrades: React.FC<{ j: Journal; limit?: number; title?: string }> = ({
  j,
  limit,
  title = "Operaciones cerradas",
}) => {
  const [filter, setFilter] = React.useState<"todas" | "tendencia" | "rebote">("todas");
  const all = j.closed.filter((t) => filter === "todas" || t.strategy === filter);
  const rows = limit ? all.slice(0, limit) : all;
  return (
    <Card>
      <div className="flex flex-wrap items-center justify-between gap-3 mb-4">
        <h3 className="text-lg font-semibold">
          {title}
          {limit && all.length > limit && (
            <span className="text-xs text-gray-500 font-normal ml-2">últimas {limit} de {all.length}</span>
          )}
        </h3>
        <div className="flex gap-1 text-xs">
          <button
            onClick={() => downloadTradesCsv(all)}
            disabled={all.length === 0}
            className="flex items-center gap-1 px-3 py-1 rounded-lg bg-dark-300 text-gray-300 hover:bg-dark-400 disabled:opacity-40 mr-2"
            title="Descargar en CSV (Excel)"
          >
            <Download className="w-3.5 h-3.5" /> CSV
          </button>
          {(["todas", "tendencia", "rebote"] as const).map((f) => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={`px-3 py-1 rounded-lg capitalize ${
                filter === f ? "bg-primary-600 text-white" : "bg-dark-300 text-gray-400 hover:bg-dark-400"
              }`}
            >
              {f}
            </button>
          ))}
        </div>
      </div>
      {rows.length === 0 ? (
        <p className="text-sm text-gray-400 py-6 text-center">Sin operaciones cerradas todavía.</p>
      ) : (
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-left text-gray-400 border-b border-dark-300">
                <th className="pb-2 pr-3">Cierre</th>
                <th className="pb-2 pr-3">Moneda</th>
                <th className="pb-2 pr-3">Estrategia</th>
                <th className="pb-2 pr-3">Entrada → Salida</th>
                <th className="pb-2 pr-3">Duración</th>
                <th className="pb-2 pr-3">Tamaño</th>
                <th className="pb-2 pr-3">Resultado</th>
                <th className="pb-2 pr-3" title="Ganancia o pérdida sobre el capital total de la cuenta">% cuenta</th>
                <th className="pb-2 pr-3">R</th>
                <th className="pb-2">Motivo</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-dark-300">
              {rows.map((t) => (
                <tr key={t.id}>
                  <td className="py-2 pr-3 text-gray-400 whitespace-nowrap">{fmtDate(t.closed_at)}</td>
                  <td className="py-2 pr-3 font-semibold">{t.symbol}</td>
                  <td className="py-2 pr-3">
                    <StrategyBadge strategy={t.strategy} paper={t.paper} />
                  </td>
                  <td className="py-2 pr-3 font-mono whitespace-nowrap">
                    {price(t.entry)} → {price(t.exit)}
                  </td>
                  <td className="py-2 pr-3">{duration(t.hours)}</td>
                  <td className="py-2 pr-3 font-mono">{usd(t.notional)}</td>
                  <td className={`py-2 pr-3 font-mono whitespace-nowrap ${pnlColor(t.pnl_pct)}`}>
                    {usd(t.pnl_usd, true)}{" "}
                    <span className="text-xs" title="Movimiento del precio">({pct(t.pnl_pct)} precio)</span>
                  </td>
                  <td className={`py-2 pr-3 font-mono font-semibold ${pnlColor(t.account_pct)}`}>
                    {pct(t.account_pct)}
                  </td>
                  <td className={`py-2 pr-3 font-mono ${pnlColor(t.r)}`}>
                    {t.r !== null ? `${t.r >= 0 ? "+" : ""}${t.r.toFixed(2)}` : "—"}
                  </td>
                  <td className="py-2 text-gray-400 text-xs">{t.reason}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Card>
  );
};

// ---------------- Página ----------------
export const JournalDashboard: React.FC = () => {
  const { data, error, loading, reload } = useJournal();

  if (loading && !data) {
    return (
      <div className="flex justify-center py-20">
        <Loader2 className="w-8 h-8 animate-spin text-primary-500" />
      </div>
    );
  }
  if (error || !data) {
    return (
      <Card className="text-center">
        <AlertCircle className="w-10 h-10 text-danger mx-auto mb-3" />
        <p className="text-gray-300 mb-4">{error ?? "Sin datos"}</p>
        <button onClick={reload} className="bg-primary-600 hover:bg-primary-700 text-white px-4 py-2 rounded-lg">
          Reintentar
        </button>
      </Card>
    );
  }

  const anyPaper = data.closed.some((t) => t.paper) || data.open.some((o) => o.paper);

  return (
    <div className="space-y-6">
      <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-gray-400">
        <span>
          {anyPaper && <span className="text-warning mr-2">● Operaciones simuladas (papel)</span>}
          Fuente: historial de señales del bot
        </span>
        <button onClick={reload} className="flex items-center gap-1 hover:text-gray-200">
          <RefreshCw className="w-3.5 h-3.5" />
          actualizado {fmtDate(data.updated_at, "HH:mm:ss")}
        </button>
      </div>
      <KpiRow j={data} />
      <div className="grid grid-cols-1 xl:grid-cols-3 gap-6">
        <div className="xl:col-span-2">
          <EquityCurve j={data} />
        </div>
        <StrategyBreakdown j={data} />
      </div>
      <OpenPositions j={data} />
      <ClosedTrades j={data} limit={10} title="Últimas operaciones" />
    </div>
  );
};

// ---------------- Vista "Historial" ----------------
export const JournalHistoryView: React.FC = () => {
  const { data, error, reload } = useJournal(60000);
  if (error) {
    return (
      <Card className="text-center">
        <p className="text-gray-300 mb-4">{error}</p>
        <button onClick={reload} className="bg-primary-600 hover:bg-primary-700 text-white px-4 py-2 rounded-lg">
          Reintentar
        </button>
      </Card>
    );
  }
  if (!data) {
    return (
      <div className="flex justify-center py-20">
        <Loader2 className="w-8 h-8 animate-spin text-primary-500" />
      </div>
    );
  }
  return (
    <div className="space-y-6">
      <OpenPositions j={data} />
      <ClosedTrades j={data} title="Todas las operaciones cerradas" />
    </div>
  );
};

// ---------------- Resultados por hora de entrada ----------------
const HourStats: React.FC<{ j: Journal }> = ({ j }) => {
  // Bloques de 4 h en hora de Chile (UTC-3)
  const blocks = [0, 4, 8, 12, 16, 20].map((start) => {
    const trades = j.closed.filter((t) => {
      const h = (new Date(t.opened_at).getUTCHours() + 21) % 24; // UTC-3
      return h >= start && h < start + 4;
    });
    const rs = trades.map((t) => t.r).filter((r): r is number => r !== null);
    return {
      label: `${String(start).padStart(2, "0")}–${String(start + 4).padStart(2, "0")} h`,
      n: trades.length,
      win: trades.length ? trades.filter((t) => t.pnl_pct > 0).length / trades.length : null,
      avgR: rs.length ? rs.reduce((a, b) => a + b, 0) / rs.length : null,
      usdSum: trades.reduce((a, t) => a + (t.pnl_usd ?? 0), 0),
    };
  });
  return (
    <Card>
      <h3 className="text-lg font-semibold mb-1">Resultados por hora de entrada</h3>
      <p className="text-xs text-gray-500 mb-4">
        Hora de Chile (UTC−3). Con pocas operaciones las diferencias son ruido: hacen falta al menos ~30 por bloque
        para sacar conclusiones. En el backtest (349 operaciones) las entradas de 01–05 h fueron las peores en ambos
        periodos.
      </p>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-gray-400 border-b border-dark-300">
              <th className="pb-2 pr-4">Entrada</th>
              <th className="pb-2 pr-4">Operaciones</th>
              <th className="pb-2 pr-4">Aciertos</th>
              <th className="pb-2 pr-4">R medio</th>
              <th className="pb-2">P&L</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-dark-300">
            {blocks.map((b) => (
              <tr key={b.label}>
                <td className="py-2 pr-4">{b.label}</td>
                <td className="py-2 pr-4">{b.n}</td>
                <td className="py-2 pr-4">{b.win !== null ? `${(b.win * 100).toFixed(0)}%` : "—"}</td>
                <td className={`py-2 pr-4 font-mono ${pnlColor(b.avgR)}`}>
                  {b.avgR !== null ? `${b.avgR >= 0 ? "+" : ""}${b.avgR.toFixed(2)}R` : "—"}
                </td>
                <td className={`py-2 font-mono ${pnlColor(b.usdSum)}`}>{b.n ? usd(b.usdSum, true) : "—"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Card>
  );
};

// ---------------- Vista "Estadísticas" ----------------
const MetricsTable: React.FC<{ j: Journal }> = ({ j }) => {
  const cols: [string, JournalStats][] = [["Total", j.stats], ...Object.entries(j.by_strategy).map(
    ([k, v]) => [STRATEGY_LABEL[k] ?? k, v] as [string, JournalStats]
  )];
  const rows: [string, (s: JournalStats) => React.ReactNode][] = [
    ["Operaciones cerradas", (s) => s.closed],
    ["Ganadas / perdidas", (s) => `${s.wins} / ${s.losses}`],
    ["Aciertos", (s) => (s.closed ? `${(s.win_rate * 100).toFixed(1)}%` : "—")],
    ["P&L realizado", (s) => <span className={pnlColor(s.total_pnl_usd)}>{usd(s.total_pnl_usd, true)}</span>],
    ["Ganancia media", (s) => `${usd(s.avg_win_usd, true)} (${pct(s.closed ? s.avg_win_pct : null)})`],
    ["Pérdida media", (s) => `${usd(s.avg_loss_usd, true)} (${pct(s.losses ? s.avg_loss_pct : null)})`],
    ["Profit factor", (s) => (s.profit_factor !== null ? s.profit_factor.toFixed(2) : s.wins ? "∞" : "—")],
    ["R medio (expectativa)", (s) => (s.expectancy_r !== null ? `${s.expectancy_r.toFixed(2)}R` : "—")],
    ["R total", (s) => `${s.total_r.toFixed(2)}R`],
    ["Mejor / peor", (s) => (s.closed ? `${pct(s.best_pct)} / ${pct(s.worst_pct)}` : "—")],
    ["Duración media", (s) => (s.closed ? duration(s.avg_hours) : "—")],
    ["Caída máxima", (s) => `${s.max_drawdown_pct.toFixed(2)}%`],
  ];
  return (
    <Card>
      <h3 className="text-lg font-semibold mb-4">Métricas detalladas</h3>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="text-left text-gray-400 border-b border-dark-300">
              <th className="pb-2 pr-4">Métrica</th>
              {cols.map(([name]) => (
                <th key={name} className="pb-2 pr-4">{name}</th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-dark-300">
            {rows.map(([label, get]) => (
              <tr key={label}>
                <td className="py-2 pr-4 text-gray-400">{label}</td>
                {cols.map(([name, st]) => (
                  <td key={name} className="py-2 pr-4 font-mono">{get(st)}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="text-xs text-gray-500 mt-3">
        R = ganancia dividida por el riesgo inicial de la operación. Con 0,5% de riesgo, +1R ≈ +0,5% de la cuenta.
      </p>
    </Card>
  );
};

export const JournalStatsView: React.FC = () => {
  const { data, error, loading, reload } = useJournal();
  if (loading && !data) {
    return (
      <div className="flex justify-center py-20">
        <Loader2 className="w-8 h-8 animate-spin text-primary-500" />
      </div>
    );
  }
  if (error || !data) {
    return (
      <Card className="text-center">
        <p className="text-gray-300 mb-4">{error ?? "Sin datos"}</p>
        <button onClick={reload} className="bg-primary-600 hover:bg-primary-700 text-white px-4 py-2 rounded-lg">
          Reintentar
        </button>
      </Card>
    );
  }
  return (
    <div className="space-y-6">
      <h2 className="text-2xl font-bold">Estadísticas detalladas</h2>
      <KpiRow j={data} />
      <EquityCurve j={data} height={320} />
      <div className="grid grid-cols-1 xl:grid-cols-3 gap-6">
        <div className="xl:col-span-2">
          <MetricsTable j={data} />
        </div>
        <StrategyBreakdown j={data} />
      </div>
      <HourStats j={data} />
    </div>
  );
};
