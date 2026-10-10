import React from "react";
import { useConfigStore } from "../../store/configStore";
import { backendApiBaseUrl } from "../../services/backendConfig";
import { Save, RotateCcw, Settings, Loader } from "lucide-react";
import toast from "react-hot-toast";

export const SystemSettings: React.FC = () => {
  const { config } = useConfigStore();
  const [exchangeType, setExchangeType] = React.useState<string>("simulated");
  const [network, setNetwork] = React.useState<string>("mainnet");
  const [logLevel, setLogLevel] = React.useState<string>("info");
  const [isEditing, setIsEditing] = React.useState(false);
  const [isSaving, setIsSaving] = React.useState(false);

  React.useEffect(() => {
    const saved = localStorage.getItem("exchange_type") || "simulated";
    setExchangeType(saved);
    setNetwork(localStorage.getItem("network") || "mainnet");
    setLogLevel(localStorage.getItem("log_level") || "info");
  }, []);

  const handleSave = async () => {
    setIsSaving(true);
    try {
      const response = await fetch(`${backendApiBaseUrl}/exchange`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          exchange_type: exchangeType,
          network: network,
          log_level: logLevel,
        }),
      });
      
      if (response.ok) {
        localStorage.setItem("exchange_type", exchangeType);
        localStorage.setItem("network", network);
        localStorage.setItem("log_level", logLevel);
        setIsEditing(false);
        toast.success("Cambios aplicados. El bot cambiará de exchange en ~10 segundos.");
      } else {
        toast.error("Error al guardar configuración");
      }
    } catch (error) {
      toast.error("Error de conexión");
    }
    setIsSaving(false);
  };

  const handleReset = () => {
    setExchangeType("simulated");
    setNetwork("mainnet");
    setLogLevel("info");
    setIsEditing(false);
  };

  const getExchangeWarning = () => {
    switch (exchangeType) {
      case "hyperliquid_mainnet":
        return "⚠️ Operando en Hyperliquid MAINNET con dinero REAL";
      case "bybit":
        return "⚠️ Operando en Bybit con dinero REAL";
      default:
        return "Modo paper trading - Sin riesgo";
    }
  };

  return (
    <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold">Configuración del Sistema</h3>
        <Settings className="w-5 h-5 text-primary-500" />
      </div>

      <div className="space-y-4">
        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Tipo de Exchange
          </label>
          <select
            value={exchangeType}
            onChange={(e) => {
              setExchangeType(e.target.value);
              setIsEditing(true);
            }}
            className="w-full bg-dark-300 rounded-lg px-4 py-2 text-sm border border-dark-400"
          >
            <option value="simulated">Simulado</option>
            <option value="hyperliquid_mainnet">Hyperliquid Mainnet</option>
            <option value="bybit">Bybit</option>
          </select>
          <p className={`text-xs mt-1 ${exchangeType !== "simulated" ? "text-warning" : "text-gray-500"}`}>
            {getExchangeWarning()}
          </p>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Red
          </label>
          <select
            value={network}
            onChange={(e) => {
              setNetwork(e.target.value);
              setIsEditing(true);
            }}
            className="w-full bg-dark-300 rounded-lg px-4 py-2 text-sm border border-dark-400"
          >
            <option value="mainnet">Mainnet</option>
            <option value="testnet">Testnet</option>
            <option value="dev">Desarrollo</option>
          </select>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Nivel de Logging
          </label>
          <select
            value={logLevel}
            onChange={(e) => {
              setLogLevel(e.target.value);
              setIsEditing(true);
            }}
            className="w-full bg-dark-300 rounded-lg px-4 py-2 text-sm border border-dark-400"
          >
            <option value="debug">Debug</option>
            <option value="info">Info</option>
            <option value="warn">Warning</option>
            <option value="error">Error</option>
          </select>
        </div>

        {isEditing && (
          <div className="flex gap-2 pt-4 border-t border-dark-300">
            <button
              onClick={handleSave}
              disabled={isSaving}
              className="flex-1 flex items-center justify-center gap-2 bg-primary-600 hover:bg-primary-700 text-white rounded-lg px-4 py-2 text-sm transition-colors disabled:opacity-50"
            >
              {isSaving ? <Loader className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />}
              {isSaving ? "Aplicando..." : "Aplicar Cambios"}
            </button>
            <button
              onClick={handleReset}
              className="flex items-center justify-center gap-2 bg-dark-300 hover:bg-dark-400 text-gray-300 rounded-lg px-4 py-2 text-sm transition-colors"
            >
              <RotateCcw className="w-4 h-4" />
              Cancelar
            </button>
          </div>
        )}
      </div>
    </div>
  );
};
