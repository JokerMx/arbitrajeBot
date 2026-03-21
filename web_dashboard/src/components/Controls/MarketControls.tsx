import React from "react";
import { useConfigStore } from "../../store/configStore";
import { Save, RotateCcw, Coins } from "lucide-react";
import toast from "react-hot-toast";

export const MarketControls: React.FC = () => {
  const { config, updateMarket } = useConfigStore();
  const [localMarkets, setLocalMarkets] = React.useState(config?.markets);
  const [isEditing, setIsEditing] = React.useState(false);

  React.useEffect(() => {
    if (config) {
      setLocalMarkets(config.markets);
    }
  }, [config]);

  if (!localMarkets) return null;

  const handleToggle = (symbol: string) => {
    const market = localMarkets.find((m) => m.symbol === symbol);
    if (market) {
      updateMarket(symbol, { enabled: !market.enabled });
    }
  };

  const handleAllocChange = (symbol: string, value: number) => {
    setLocalMarkets(
      localMarkets.map((m) =>
        m.symbol === symbol ? { ...m, marginAlloc: value } : m
      )
    );
    setIsEditing(true);
  };

  const handleSave = async () => {
    try {
      for (const market of localMarkets) {
        await updateMarket(market.symbol, {
          marginAlloc: market.marginAlloc,
          minSize: market.minSize,
        });
      }
      setIsEditing(false);
      toast.success("Mercados actualizados");
    } catch (error) {
      toast.error("Error al guardar");
    }
  };

  const handleReset = () => {
    if (config) {
      setLocalMarkets(config.markets);
      setIsEditing(false);
    }
  };

  return (
    <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold">Mercados</h3>
        <Coins className="w-5 h-5 text-warning" />
      </div>

      <div className="space-y-4">
        {localMarkets.map((market) => (
          <div
            key={market.symbol}
            className="p-4 bg-dark-300 rounded-lg border border-dark-400"
          >
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-3">
                <button
                  onClick={() => handleToggle(market.symbol)}
                  className={`relative inline-flex h-5 w-9 items-center rounded-full transition-colors ${
                    market.enabled ? "bg-success" : "bg-dark-400"
                  }`}
                >
                  <span
                    className={`inline-block h-3 w-3 transform rounded-full bg-white transition-transform ${
                      market.enabled ? "translate-x-5" : "translate-x-1"
                    }`}
                  />
                </button>
                <span className="font-semibold">{market.symbol}</span>
              </div>
              <span
                className={`text-xs px-2 py-1 rounded-full ${
                  market.enabled
                    ? "bg-success/20 text-success"
                    : "bg-dark-400 text-gray-400"
                }`}
              >
                {market.enabled ? "Activo" : "Inactivo"}
              </span>
            </div>

            {market.enabled && (
              <div>
                <label className="block text-xs text-gray-400 mb-1">
                  Asignación de Capital: {(market.marginAlloc * 100).toFixed(0)}%
                </label>
                <input
                  type="range"
                  min="0.05"
                  max="0.25"
                  step="0.01"
                  value={market.marginAlloc}
                  onChange={(e) =>
                    handleAllocChange(market.symbol, parseFloat(e.target.value))
                  }
                  disabled={!market.enabled}
                  className="w-full"
                />
              </div>
            )}
          </div>
        ))}

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