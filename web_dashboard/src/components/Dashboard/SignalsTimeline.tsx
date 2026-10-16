import React from "react";
import { format, parseISO } from "date-fns";
import { es } from "date-fns/locale";
import { Loader2, RefreshCw } from "lucide-react";
import { backendApiBaseUrl } from "../../services/backendConfig";
import { price, pct, STRATEGY_LABEL } from "../../services/journal";

interface SignalRecord {
  id: number;
  at: string;
  free_released: boolean;
  event: {
    kind: "Entry" | "StopMoved" | "Exit";
    symbol: string;
    paper: boolean;
    strategy?: string;
    price?: string;
    stop?: string;
    entry?: string;
    exit?: string;
    reason?: string;
    notional?: string;
  };
}

const n = (v?: string) => (v === undefined ? undefined : Number(v));

/** Historial de señales del bot (entradas, stops movidos y cierres), más recientes primero */
export const SignalsTimeline: React.FC = () => {
  const [records, setRecords] = React.useState<SignalRecord[] | null>(null);
  const [error, setError] = React.useState(false);

  const load = React.useCallback(async () => {
    try {
      const r = await fetch(`${backendApiBaseUrl}/signals`);
      if (!r.ok) throw new Error();
      setRecords(await r.json());
      setError(false);
    } catch {
      setError(true);
    }
  }, []);

  React.useEffect(() => {
    load();
    const id = setInterval(load, 30000);
    return () => clearInterval(id);
  }, [load]);

  if (error) return <p className="text-danger text-sm">No se pudieron cargar las señales.</p>;
  if (!records)
    return (
      <div className="flex justify-center py-10">
        <Loader2 className="w-6 h-6 animate-spin text-primary-500" />
      </div>
    );

  return (
    <div>
      <div className="flex justify-between items-center mb-4 text-xs text-gray-400">
        <span>{records.length} señales · se actualiza cada 30 s · también llegan por Telegram</span>
        <button onClick={load} className="flex items-center gap-1 hover:text-gray-200">
          <RefreshCw className="w-3.5 h-3.5" /> actualizar
        </button>
      </div>
      {records.length === 0 ? (
        <p className="text-gray-400 text-center py-8">
          Aún no hay señales. Aparecerán aquí cada entrada, movimiento de stop y cierre.
        </p>
      ) : (
        <ol className="relative border-l border-dark-400 ml-2 space-y-4">
          {records.map((r) => {
            const e = r.event;
            const strat = e.strategy === "rebote" ? "rebote" : "tendencia";
            const pnl =
              e.kind === "Exit" && n(e.entry) ? ((n(e.exit)! / n(e.entry)! - 1) * 100) : undefined;
            const [icon, color, title] =
              e.kind === "Entry"
                ? [strat === "rebote" ? "🔄" : "🚀", "bg-primary-500", `Entrada ${e.symbol}`]
                : e.kind === "StopMoved"
                ? ["🛡️", "bg-info", `Stop subido en ${e.symbol}`]
                : [pnl !== undefined && pnl >= 0 ? "✅" : "🔻", pnl !== undefined && pnl >= 0 ? "bg-success" : "bg-danger", `Cierre ${e.symbol}`];
            return (
              <li key={r.id} className="ml-4">
                <span className={`absolute -left-1.5 mt-1.5 w-3 h-3 rounded-full ${color}`} />
                <div className="flex flex-wrap items-center gap-2">
                  <span className="text-xs text-gray-500">{format(parseISO(r.at), "dd/MM/yyyy HH:mm", { locale: es })}</span>
                  <span className="font-semibold">
                    {icon} {title}
                  </span>
                  <span className="text-[11px] px-2 py-0.5 rounded-full bg-dark-300 text-gray-300">
                    {STRATEGY_LABEL[strat]}
                  </span>
                  {e.paper && <span className="text-[10px] px-1.5 py-0.5 rounded bg-dark-400 text-gray-400">papel</span>}
                </div>
                <p className="text-sm text-gray-300 mt-1">
                  {e.kind === "Entry" && (
                    <>
                      Compra a {price(n(e.price))} · stop {price(n(e.stop))}
                      {e.notional && <> · tamaño ${Number(e.notional).toFixed(2)}</>}
                    </>
                  )}
                  {e.kind === "StopMoved" && <>Nuevo stop {price(n(e.stop))}</>}
                  {e.kind === "Exit" && (
                    <>
                      {price(n(e.entry))} → {price(n(e.exit))} ·{" "}
                      <span className={pnl !== undefined && pnl >= 0 ? "text-success" : "text-danger"}>{pct(pnl)}</span> ·{" "}
                      {e.reason}
                    </>
                  )}
                </p>
              </li>
            );
          })}
        </ol>
      )}
    </div>
  );
};
