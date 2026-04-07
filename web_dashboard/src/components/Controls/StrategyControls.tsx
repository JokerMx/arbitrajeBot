import React from "react";
import { useConfigStore } from "../../store/configStore";
import { Save, RotateCcw, TrendingUp } from "lucide-react";
import toast from "react-hot-toast";

export const StrategyControls: React.FC = () => {
  const { config, updateStrategy } = useConfigStore();
  const [localStrategy, setLocalStrategy] = React.useState(config?.strategy);
  const [isEditing, setIsEditing] = React.useState(false);

  React.useEffect(() => {
    if (config) {
      setLocalStrategy(config.strategy);
    }
  }, [config]);

  if (!localStrategy) return null;

  const handleChange = (field: keyof typeof localStrategy, value: number) => {
    setLocalStrategy({ ...localStrategy, [field]: value });
    setIsEditing(true);
  };

  const handleSave = async () => {
    try {
      await updateStrategy(localStrategy);
      setIsEditing(false);
      toast.success("Estrategia actualizada");
    } catch (error) {
      toast.error("Error al guardar");
    }
  };

  const handleReset = () => {
    if (config) {
      setLocalStrategy(config.strategy);
      setIsEditing(false);
    }
  };

  return (
    <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold">Estrategia Avellaneda-Stoikov</h3>
        <TrendingUp className="w-5 h-5 text-primary-500" />
      </div>

      <div className="space-y-4">
        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Spread Base (ticks)
          </label>
          <input
            type="range"
            min="1"
            max="20"
            value={localStrategy.baseSpreadTicks}
            onChange={(e) =>
              handleChange("baseSpreadTicks", parseInt(e.target.value))
            }
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-500 mt-1">
            <span>Agresivo (1)</span>
            <span>{localStrategy.baseSpreadTicks}</span>
            <span>Conservador (20)</span>
          </div>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Aversión al Riesgo (γ)
          </label>
          <input
            type="range"
            min="0.01"
            max="1"
            step="0.01"
            value={localStrategy.riskAversion}
            onChange={(e) =>
              handleChange("riskAversion", parseFloat(e.target.value))
            }
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-500 mt-1">
            <span>Agresivo</span>
            <span>{localStrategy.riskAversion.toFixed(2)}</span>
            <span>Conservador</span>
          </div>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Impacto de Mercado (k)
          </label>
          <input
            type="range"
            min="0.001"
            max="0.1"
            step="0.001"
            value={localStrategy.marketImpact}
            onChange={(e) =>
              handleChange("marketImpact", parseFloat(e.target.value))
            }
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-500 mt-1">
            <span>Bajo</span>
            <span>{localStrategy.marketImpact.toFixed(3)}</span>
            <span>Alto</span>
          </div>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Fuerza Skew Inventario
          </label>
          <input
            type="range"
            min="0"
            max="1"
            step="0.1"
            value={localStrategy.inventorySkewStrength}
            onChange={(e) =>
              handleChange("inventorySkewStrength", parseFloat(e.target.value))
            }
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-500 mt-1">
            <span>Neutral</span>
            <span>{localStrategy.inventorySkewStrength.toFixed(1)}</span>
            <span>Agresivo</span>
          </div>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Refresco de Órdenes (ms)
          </label>
          <select
            value={localStrategy.orderRefreshMs}
            onChange={(e) =>
              handleChange("orderRefreshMs", parseInt(e.target.value))
            }
            className="w-full bg-dark-300 rounded-lg px-4 py-2 text-sm border border-dark-400"
          >
            <option value={10000}>10 segundos</option>
            <option value={20000}>20 segundos</option>
            <option value={30000}>30 segundos</option>
            <option value={60000}>1 minuto</option>
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