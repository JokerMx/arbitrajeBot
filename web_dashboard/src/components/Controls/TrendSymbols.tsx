import React from "react";
import { TrendingUp, X, Plus, Save, RotateCcw, Loader2, Search } from "lucide-react";
import toast from "react-hot-toast";
import { backendApiBaseUrl } from "../../services/backendConfig";

interface TrendSymbolsState {
  enabled: boolean;
  active: string[];
  source: "dashboard" | "env";
  env_default: string[];
  available: string[];
  max: number;
}

// Monedas líquidas usadas en los backtests (atajo de selección)
const LIQUID = ["BTC", "ETH", "SOL", "HYPE", "XRP", "SUI", "ENA", "NEAR", "ZEC", "WLD"];

export const TrendSymbols: React.FC = () => {
  const [data, setData] = React.useState<TrendSymbolsState | null>(null);
  const [selected, setSelected] = React.useState<string[]>([]);
  const [query, setQuery] = React.useState("");
  const [saving, setSaving] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const load = React.useCallback(async () => {
    try {
      const r = await fetch(`${backendApiBaseUrl}/trend/symbols`);
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      const d: TrendSymbolsState = await r.json();
      setData(d);
      setSelected(d.active);
      setError(null);
    } catch (e) {
      setError("No se pudo cargar la lista de monedas");
    }
  }, []);

  React.useEffect(() => {
    load();
  }, [load]);

  if (error) {
    return (
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300 text-sm text-danger">{error}</div>
    );
  }
  if (!data) {
    return (
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300 flex justify-center">
        <Loader2 className="w-6 h-6 animate-spin text-primary-500" />
      </div>
    );
  }
  if (!data.enabled) {
    return (
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300 text-sm text-gray-400">
        La estrategia de tendencia no está activa (STRATEGY_MODE=market_making). Las monedas se controlan
        con los mercados de abajo.
      </div>
    );
  }

  const dirty =
    selected.length !== data.active.length || selected.some((s, i) => s !== data.active[i]);
  const q = query.trim().toUpperCase();
  const suggestions = q
    ? data.available.filter((s) => s.includes(q) && !selected.includes(s)).slice(0, 12)
    : [];

  const add = (symbol: string) => {
    if (selected.includes(symbol)) return;
    if (selected.length >= data.max) {
      toast.error(`Máximo ${data.max} monedas`);
      return;
    }
    setSelected([...selected, symbol]);
    setQuery("");
  };
  const remove = (symbol: string) => setSelected(selected.filter((s) => s !== symbol));

  const save = async () => {
    if (selected.length === 0) {
      toast.error("Selecciona al menos una moneda");
      return;
    }
    setSaving(true);
    try {
      const r = await fetch(`${backendApiBaseUrl}/trend/symbols`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ symbols: selected }),
      });
      const res = await r.json();
      res.success ? toast.success(res.message) : toast.error(res.message);
      await load();
    } catch {
      toast.error("Error al guardar");
    } finally {
      setSaving(false);
    }
  };

  const reset = async () => {
    if (!window.confirm("¿Volver a las monedas de TREND_SYMBOLS del .env?")) return;
    setSaving(true);
    try {
      const r = await fetch(`${backendApiBaseUrl}/trend/symbols`, { method: "DELETE" });
      const res = await r.json();
      res.success ? toast.success(res.message) : toast.error(res.message);
      await load();
    } catch {
      toast.error("Error al restablecer");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="bg-dark-200 rounded-xl p-6 border border-dark-300 space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-lg font-semibold">Monedas de la estrategia de tendencia</h3>
          <p className="text-xs text-gray-400 mt-1">
            Origen:{" "}
            {data.source === "dashboard" ? (
              <span className="text-primary-400">selección del dashboard</span>
            ) : (
              <span className="text-gray-300">TREND_SYMBOLS del .env</span>
            )}{" "}
            · los cambios se aplican en la próxima evaluación horaria, sin reiniciar
          </p>
        </div>
        <TrendingUp className="w-5 h-5 text-success" />
      </div>

      {/* Seleccionadas */}
      <div className="flex flex-wrap gap-2">
        {selected.length === 0 && <span className="text-sm text-gray-500">Ninguna moneda seleccionada</span>}
        {selected.map((s) => (
          <span
            key={s}
            className="flex items-center gap-1 bg-primary-500/15 text-primary-300 border border-primary-500/30 rounded-full pl-3 pr-1 py-1 text-sm"
          >
            {s}
            <button
              onClick={() => remove(s)}
              className="p-0.5 rounded-full hover:bg-primary-500/30"
              title={`Quitar ${s}`}
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </span>
        ))}
      </div>

      {/* Buscador */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" />
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && suggestions.length > 0) add(suggestions[0]);
          }}
          placeholder={`Buscar entre ${data.available.length} mercados de Hyperliquid (ej: DOGE)`}
          className="w-full bg-dark-300 border border-dark-400 rounded-lg pl-9 pr-4 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-primary-500"
        />
        {suggestions.length > 0 && (
          <div className="absolute z-10 mt-1 w-full bg-dark-300 border border-dark-400 rounded-lg shadow-lg max-h-60 overflow-auto">
            {suggestions.map((s) => (
              <button
                key={s}
                onClick={() => add(s)}
                className="w-full text-left px-4 py-2 text-sm hover:bg-dark-400 flex items-center gap-2"
              >
                <Plus className="w-3.5 h-3.5 text-success" />
                {s}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Atajos */}
      <div className="flex flex-wrap gap-2 text-xs">
        <button
          onClick={() => setSelected(LIQUID.filter((s) => data.available.includes(s)))}
          className="px-3 py-1 rounded-lg bg-dark-300 hover:bg-dark-400 text-gray-300"
        >
          Líquidas del backtest ({LIQUID.length})
        </button>
        {data.env_default.length > 0 && (
          <button
            onClick={() => setSelected(data.env_default.filter((s) => data.available.includes(s)))}
            className="px-3 py-1 rounded-lg bg-dark-300 hover:bg-dark-400 text-gray-300"
          >
            Las del .env ({data.env_default.length})
          </button>
        )}
      </div>

      <p className="text-xs text-gray-500">
        Si quitas una moneda con posición abierta, el bot la sigue gestionando (stop y salida) hasta cerrarla;
        solo deja de abrir nuevas. Más monedas = más señales, pero la estrategia se validó con monedas líquidas.
      </p>

      <div className="flex gap-2 pt-2 border-t border-dark-300">
        <button
          onClick={save}
          disabled={!dirty || saving}
          className="flex-1 flex items-center justify-center gap-2 bg-primary-600 hover:bg-primary-700 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg px-4 py-2 text-sm transition-colors"
        >
          {saving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />}
          Guardar ({selected.length})
        </button>
        {dirty && (
          <button
            onClick={() => setSelected(data.active)}
            className="px-4 py-2 rounded-lg bg-dark-300 hover:bg-dark-400 text-gray-300 text-sm"
          >
            Deshacer
          </button>
        )}
        {data.source === "dashboard" && (
          <button
            onClick={reset}
            disabled={saving}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-dark-300 hover:bg-dark-400 text-gray-300 text-sm"
            title="Borrar la selección del dashboard y usar TREND_SYMBOLS del .env"
          >
            <RotateCcw className="w-4 h-4" />
            Usar .env
          </button>
        )}
      </div>
    </div>
  );
};
