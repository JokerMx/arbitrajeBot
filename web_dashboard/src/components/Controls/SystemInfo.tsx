import React from "react";
import { Server, Save, RotateCw, Loader2, ShieldAlert } from "lucide-react";
import toast from "react-hot-toast";
import { backendApiBaseUrl } from "../../services/backendConfig";
import { restartBot, formatUptime } from "../../services/botControl";

interface SystemData {
  exchange_type: string;
  live: boolean;
  rebound_live: boolean;
  strategy_mode: string;
  network: string;
  wallet: string;
  initial_capital: number;
  data_dir: string;
  log_level: string;
  api_token_set: boolean;
  uptime_secs: number;
  version: string;
}

const Row: React.FC<{ label: string; children: React.ReactNode }> = ({ label, children }) => (
  <div className="flex justify-between gap-4 py-2 text-sm">
    <span className="text-gray-400">{label}</span>
    <span className="text-right font-mono">{children}</span>
  </div>
);

/**
 * Estado del sistema (solo lectura) + capital inicial.
 * El paso a dinero real se hace en el .env a propósito: no hay botón web para eso.
 */
export const SystemInfo: React.FC = () => {
  const [d, setD] = React.useState<SystemData | null>(null);
  const [capital, setCapital] = React.useState("");
  const [saving, setSaving] = React.useState(false);
  const [needsRestart, setNeedsRestart] = React.useState(false);

  const load = React.useCallback(async () => {
    try {
      const r = await fetch(`${backendApiBaseUrl}/system`);
      const data: SystemData = await r.json();
      setD(data);
      setCapital(String(data.initial_capital));
    } catch {
      toast.error("No se pudo leer el estado del sistema");
    }
  }, []);

  React.useEffect(() => {
    load();
  }, [load]);

  const saveCapital = async () => {
    setSaving(true);
    try {
      const r = await fetch(`${backendApiBaseUrl}/system/capital`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ capital: Number(capital) }),
      });
      const res = await r.json();
      res.success ? toast.success(res.message) : toast.error(res.message);
      if (res.restart_required) setNeedsRestart(true);
    } finally {
      setSaving(false);
    }
  };

  if (!d) {
    return (
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300 flex justify-center">
        <Loader2 className="w-6 h-6 animate-spin text-primary-500" />
      </div>
    );
  }

  return (
    <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-lg font-semibold">Sistema</h3>
        <Server className="w-5 h-5 text-gray-400" />
      </div>
      <div className="divide-y divide-dark-300">
        <Row label="Modo de operación">
          <span className={d.live ? "text-danger" : "text-warning"}>{d.live ? "REAL (dinero real)" : "PAPEL (simulado)"}</span>
        </Row>
        <Row label="Estrategias">
          {d.strategy_mode === "trend" ? (
            <>
              tendencia {d.live ? "real" : "papel"} · rebote {d.rebound_live ? "real" : "papel"}
            </>
          ) : (
            "market making"
          )}
        </Row>
        <Row label="Exchange / red">
          {d.exchange_type} · {d.network || "—"}
        </Row>
        <Row label="Wallet">{d.wallet}</Row>
        <Row label="Activo hace">{formatUptime(d.uptime_secs)}</Row>
        <Row label="Versión">v{d.version}</Row>
        <Row label="Datos">{d.data_dir}</Row>
        <Row label="Token de la API">
          {d.api_token_set ? <span className="text-success">configurado</span> : <span className="text-danger">sin token</span>}
        </Row>
      </div>

      <div className="mt-4 p-3 rounded-lg bg-dark-300/60 border border-dark-400 text-xs text-gray-400 flex gap-2">
        <ShieldAlert className="w-4 h-4 text-warning shrink-0" />
        <span>
          Por seguridad, el paso a dinero real se cambia en el archivo <code>.env</code> (EXCHANGE_TYPE y REBOUND_LIVE)
          y luego se reinicia el bot. No hay un botón web que active el dinero real.
        </span>
      </div>

      <div className="mt-4">
        <label className="block text-sm text-gray-400 mb-1">Capital inicial {d.live ? "" : "(simulado)"}</label>
        <div className="flex gap-2">
          <input
            type="number"
            min={10}
            step={10}
            value={capital}
            onChange={(e) => setCapital(e.target.value)}
            className="flex-1 bg-dark-300 border border-dark-400 rounded-lg px-3 py-2 text-sm font-mono focus:outline-none focus:ring-2 focus:ring-primary-500"
          />
          <button
            onClick={saveCapital}
            disabled={saving || Number(capital) === d.initial_capital}
            className="flex items-center gap-2 bg-primary-600 hover:bg-primary-700 disabled:opacity-50 text-white rounded-lg px-4 py-2 text-sm"
          >
            {saving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />} Guardar
          </button>
        </div>
        <p className="text-xs text-gray-500 mt-1">
          Base para el tamaño de las posiciones y las estadísticas. En modo real se usa el saldo de la cuenta.
        </p>
        {needsRestart && (
          <button
            onClick={async () => {
              if (window.confirm("¿Reiniciar el bot ahora para aplicar el nuevo capital?")) await restartBot();
            }}
            className="mt-3 w-full flex items-center justify-center gap-2 bg-warning/20 hover:bg-warning/30 text-warning rounded-lg px-4 py-2 text-sm"
          >
            <RotateCw className="w-4 h-4" /> Reiniciar para aplicar
          </button>
        )}
      </div>
    </div>
  );
};
