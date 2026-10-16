import React from "react";
import { TrendingUp, RefreshCcw, Save, RotateCcw, Loader2 } from "lucide-react";
import toast from "react-hot-toast";
import { backendApiBaseUrl } from "../../services/backendConfig";

type Values = Record<string, number | boolean>;

interface StrategyState {
  enabled: boolean;
  active: Values;
  env_default: Values;
  source: "dashboard" | "env";
}

// Campos editables: [clave, etiqueta, tipo, mín, máx, paso, ayuda]
type Field = [string, string, "pct" | "num" | "bool", number, number, number, string];

const TREND: Field[] = [
  ["trend_risk_per_trade", "Riesgo por operación", "pct", 0.1, 2, 0.1, "% del capital que se pierde si salta el stop"],
  ["trend_max_risk_per_trade", "Riesgo máx. al subir a $10", "pct", 0.1, 2, 0.05, "Tope cuando la posición se sube al mínimo del exchange"],
  ["trend_max_positions", "Posiciones máximas", "num", 1, 10, 1, "Operaciones de tendencia abiertas a la vez"],
  ["trend_max_leverage", "Apalancamiento total máx.", "num", 1, 5, 0.5, "Nocional total / capital (compartido con rebote)"],
  ["trend_breakout_hours", "Canal de entrada (horas)", "num", 24, 1000, 1, "Compra al romper el máximo de estas horas (168 = 7 días)"],
  ["trend_exit_hours", "Canal de salida (horas)", "num", 6, 500, 1, "Vende si cierra bajo el mínimo de estas horas"],
  ["trend_stop_atr", "Stop (ATR)", "num", 1, 6, 0.5, "Distancia del stop en múltiplos de la volatilidad"],
];

const REBOUND: Field[] = [
  ["rebound_enabled", "Estrategia activa", "bool", 0, 1, 1, "Compra en sobreventa y vende al rebote"],
  ["rebound_risk_per_trade", "Riesgo por operación", "pct", 0.1, 2, 0.1, "% del capital que se pierde si salta el stop"],
  ["rebound_max_positions", "Posiciones máximas", "num", 0, 5, 1, "Operaciones de rebote abiertas a la vez"],
  ["rebound_entry_rsi", "Comprar con RSI menor a", "num", 5, 40, 1, "Sobreventa extrema (validado: 20)"],
  ["rebound_exit_rsi", "Vender con RSI mayor a", "num", 30, 80, 1, "Rebote completado (validado: 50)"],
  ["rebound_max_hold_hours", "Tiempo máximo (horas)", "num", 4, 240, 1, "Cierra si no rebota en este tiempo"],
];

const FieldInput: React.FC<{
  f: Field;
  value: number | boolean;
  def: number | boolean;
  onChange: (v: number | boolean) => void;
}> = ({ f, value, def, onChange }) => {
  const [key, label, kind, min, max, step, help] = f;
  const changed = value !== def;
  if (kind === "bool") {
    return (
      <div className="flex items-center justify-between py-2">
        <div>
          <p className="text-sm">{label}</p>
          <p className="text-xs text-gray-500">{help}</p>
        </div>
        <button
          onClick={() => onChange(!value)}
          className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
            value ? "bg-success" : "bg-dark-400"
          }`}
        >
          <span
            className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
              value ? "translate-x-6" : "translate-x-1"
            }`}
          />
        </button>
      </div>
    );
  }
  const shown = kind === "pct" ? Number(((value as number) * 100).toFixed(3)) : (value as number);
  return (
    <div className="py-2">
      <div className="flex items-center justify-between gap-3">
        <label htmlFor={key} className="text-sm">
          {label}
          {changed && <span className="ml-2 text-[10px] text-primary-300">(env: {kind === "pct" ? `${((def as number) * 100).toFixed(2)}%` : String(def)})</span>}
        </label>
        <div className="flex items-center gap-1">
          <input
            id={key}
            type="number"
            min={min}
            max={max}
            step={step}
            value={shown}
            onChange={(e) => {
              const n = parseFloat(e.target.value);
              if (Number.isNaN(n)) return;
              const clamped = Math.min(max, Math.max(min, n));
              onChange(kind === "pct" ? clamped / 100 : clamped);
            }}
            className="w-24 bg-dark-300 border border-dark-400 rounded-lg px-2 py-1 text-sm text-right font-mono focus:outline-none focus:ring-2 focus:ring-primary-500"
          />
          <span className="text-xs text-gray-400 w-4">{kind === "pct" ? "%" : ""}</span>
        </div>
      </div>
      <p className="text-xs text-gray-500 mt-0.5">{help}</p>
    </div>
  );
};

export const StrategySettings: React.FC = () => {
  const [data, setData] = React.useState<StrategyState | null>(null);
  const [form, setForm] = React.useState<Values>({});
  const [saving, setSaving] = React.useState(false);

  const load = React.useCallback(async () => {
    try {
      const r = await fetch(`${backendApiBaseUrl}/strategy/config`);
      const d: StrategyState = await r.json();
      setData(d);
      if (d.enabled) setForm(d.active);
    } catch {
      toast.error("No se pudo cargar la configuración de estrategias");
    }
  }, []);

  React.useEffect(() => {
    load();
  }, [load]);

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
        El bot está en modo market making (STRATEGY_MODE=market_making). Usa los parámetros de abajo.
      </div>
    );
  }

  const dirty = Object.keys(form).some((k) => form[k] !== data.active[k]);
  const set = (k: string, v: number | boolean) => setForm({ ...form, [k]: v });

  const save = async () => {
    setSaving(true);
    try {
      const body: Values = {};
      [...TREND, ...REBOUND].forEach(([k]) => (body[k] = form[k]));
      const r = await fetch(`${backendApiBaseUrl}/strategy/config`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
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
    if (!window.confirm("¿Volver a los valores del .env?")) return;
    setSaving(true);
    try {
      const r = await fetch(`${backendApiBaseUrl}/strategy/config`, { method: "DELETE" });
      const res = await r.json();
      toast.success(res.message);
      await load();
    } finally {
      setSaving(false);
    }
  };

  const panel = (title: string, icon: React.ReactNode, fields: Field[], footer?: React.ReactNode) => (
    <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-lg font-semibold">{title}</h3>
        {icon}
      </div>
      <div className="divide-y divide-dark-300">
        {fields.map((f) => (
          <FieldInput key={f[0]} f={f} value={form[f[0]]} def={data.env_default[f[0]]} onChange={(v) => set(f[0], v)} />
        ))}
      </div>
      {footer}
    </div>
  );

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-2 text-xs text-gray-400">
        <span>
          Origen: {data.source === "dashboard" ? <span className="text-primary-300">ajustes del dashboard</span> : "valores del .env"} ·
          los cambios se aplican en la próxima evaluación horaria, sin reiniciar
        </span>
      </div>
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {panel("🚀 Tendencia", <TrendingUp className="w-5 h-5 text-primary-400" />, TREND)}
        {panel(
          "🔄 Rebote",
          <RefreshCcw className="w-5 h-5 text-info" />,
          REBOUND,
          <p className="text-xs mt-3 text-gray-400">
            Modo: <span className={data.active.rebound_live ? "text-danger" : "text-warning"}>
              {data.active.rebound_live ? "REAL" : "PAPEL"}
            </span>{" "}
            · operar con dinero real solo se habilita en el .env (REBOUND_LIVE=true).
          </p>
        )}
      </div>
      <div className="flex gap-2">
        <button
          onClick={save}
          disabled={!dirty || saving}
          className="flex-1 flex items-center justify-center gap-2 bg-primary-600 hover:bg-primary-700 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg px-4 py-2 text-sm"
        >
          {saving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />}
          Guardar cambios
        </button>
        {dirty && (
          <button onClick={() => setForm(data.active)} className="px-4 py-2 rounded-lg bg-dark-300 hover:bg-dark-400 text-sm">
            Deshacer
          </button>
        )}
        {data.source === "dashboard" && (
          <button
            onClick={reset}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-dark-300 hover:bg-dark-400 text-sm"
          >
            <RotateCcw className="w-4 h-4" /> Usar .env
          </button>
        )}
      </div>
    </div>
  );
};
