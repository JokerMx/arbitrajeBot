import React from "react";
import { useConfigStore } from "../../store/configStore";
import { Save, RotateCcw, Shield } from "lucide-react";
import toast from "react-hot-toast";

export const RiskControls: React.FC = () => {
  const { config, updateRisk } = useConfigStore();
  const [localRisk, setLocalRisk] = React.useState(config?.risk);
  const [isEditing, setIsEditing] = React.useState(false);

  React.useEffect(() => {
    if (config) {
      setLocalRisk(config.risk);
    }
  }, [config]);

  if (!localRisk) return null;

  const safeMultiply = (value: number | undefined, factor: number): number => {
    return (value ?? 0) * factor;
  };

  const handleChange = (field: keyof typeof localRisk, value: number | boolean) => {
    setLocalRisk({ ...localRisk, [field]: value });
    setIsEditing(true);
  };

  const handleSave = async () => {
    try {
      await updateRisk(localRisk);
      setIsEditing(false);
      toast.success("Configuración de riesgo actualizada");
    } catch (error) {
      toast.error("Error al guardar");
    }
  };

  const handleReset = () => {
    if (config) {
      setLocalRisk(config.risk);
      setIsEditing(false);
    }
  };

  return (
    <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold">Gestión de Riesgo</h3>
        <Shield className="w-5 h-5 text-warning" />
      </div>

      <div className="space-y-4">
        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Máx Posición (% Capital)
          </label>
          <input
            type="range"
            min="0.05"
            max="0.30"
            step="0.01"
            value={localRisk.maxPositionSizePct}
            onChange={(e) =>
              handleChange("maxPositionSizePct", parseFloat(e.target.value))
            }
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-500 mt-1">
            <span>5%</span>
            <span>{safeMultiply(localRisk.maxPositionSizePct, 100).toFixed(0)}%</span>
            <span>30%</span>
          </div>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Pérdida Diaria Máx (% Capital)
          </label>
          <input
            type="range"
            min="0.05"
            max="0.50"
            step="0.05"
            value={localRisk.maxDailyLossPct}
            onChange={(e) =>
              handleChange("maxDailyLossPct", parseFloat(e.target.value))
            }
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-500 mt-1">
            <span>5%</span>
            <span>{safeMultiply(localRisk.maxDailyLossPct, 100).toFixed(0)}%</span>
            <span>50%</span>
          </div>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Drawdown Máx (%)
          </label>
          <input
            type="range"
            min="0.05"
            max="0.30"
            step="0.01"
            value={localRisk.maxDrawdownPct}
            onChange={(e) =>
              handleChange("maxDrawdownPct", parseFloat(e.target.value))
            }
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-500 mt-1">
            <span>5%</span>
            <span>{safeMultiply(localRisk.maxDrawdownPct, 100).toFixed(0)}%</span>
            <span>30%</span>
          </div>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Circuit Breaker (%)
          </label>
          <input
            type="range"
            min="0.01"
            max="0.10"
            step="0.01"
            value={localRisk.circuitBreakerThreshold}
            onChange={(e) =>
              handleChange("circuitBreakerThreshold", parseFloat(e.target.value))
            }
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-500 mt-1">
            <span>1%</span>
            <span>{safeMultiply(localRisk.circuitBreakerThreshold, 100).toFixed(0)}%</span>
            <span>10%</span>
          </div>
        </div>

        <div className="flex items-center justify-between">
          <label className="text-sm text-gray-400">Kill Switch</label>
          <button
            onClick={() => handleChange("killSwitchEnabled", !localRisk.killSwitchEnabled)}
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              localRisk.killSwitchEnabled ? "bg-success" : "bg-dark-300"
            }`}
          >
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                localRisk.killSwitchEnabled ? "translate-x-6" : "translate-x-1"
              }`}
            />
          </button>
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