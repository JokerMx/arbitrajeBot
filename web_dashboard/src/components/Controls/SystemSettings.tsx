import React from "react";
import { useConfigStore } from "../../store/configStore";
import { Save, RotateCcw, Settings } from "lucide-react";
import toast from "react-hot-toast";

export const SystemSettings: React.FC = () => {
  const { config } = useConfigStore();
  const [exchangeType, setExchangeType] = React.useState<string>(
    localStorage.getItem("exchange_type") || "simulated"
  );
  const [network, setNetwork] = React.useState<string>(
    localStorage.getItem("network") || "mainnet"
  );
  const [logLevel, setLogLevel] = React.useState<string>(
    localStorage.getItem("log_level") || "info"
  );
  const [isEditing, setIsEditing] = React.useState(false);

  const handleSave = () => {
    localStorage.setItem("exchange_type", exchangeType);
    localStorage.setItem("network", network);
    localStorage.setItem("log_level", logLevel);
    setIsEditing(false);
    toast.success("Configuración del sistema actualizada");
  };

  const handleReset = () => {
    setExchangeType("simulated");
    setNetwork("mainnet");
    setLogLevel("info");
    setIsEditing(false);
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
          </select>
          <p className="text-xs text-gray-500 mt-1">
            {exchangeType === "simulated"
              ? "Modo paper trading - Sin riesgo"
              : "⚠️ Operando con dinero real"}
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
              className="flex-1 flex items-center justify-center gap-2 bg-primary-600 hover:bg-primary-700 text-white rounded-lg px-4 py-2 text-sm transition-colors"
            >
              <Save className="w-4 h-4" />
              Guardar Cambios
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
