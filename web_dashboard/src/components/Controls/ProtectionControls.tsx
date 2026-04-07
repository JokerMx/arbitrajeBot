import React, { useState } from "react";
import { useConfigStore } from "../../store/configStore";
import { Save, RotateCcw, Shield, AlertTriangle } from "lucide-react";
import toast from "react-hot-toast";

export const ProtectionControls: React.FC = () => {
  const { config, updateProtection } = useConfigStore();
  const [localProtection, setLocalProtection] = useState(config.protection);
  const [isEditing, setIsEditing] = React.useState(false);

  React.useEffect(() => {
    setLocalProtection(config.protection);
  }, [config.protection]);

  const handleChange = (field: keyof typeof localProtection, value: number | boolean) => {
    setLocalProtection({ ...localProtection, [field]: value });
    setIsEditing(true);
  };

  const safeMultiply = (value: number | undefined, factor: number): number => {
    return (value ?? 0) * factor;
  };

  const handleSave = async () => {
    try {
      await updateProtection(localProtection);
      setIsEditing(false);
      toast.success("Protección de capital actualizada");
    } catch (error) {
      toast.error("Error al guardar");
    }
  };

  const handleReset = () => {
    setLocalProtection(config.protection);
    setIsEditing(false);
  };

  return (
    <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold">Protección de Capital</h3>
        <Shield className="w-5 h-5 text-success" />
      </div>

      <div className="space-y-4">
        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Capital en Riesgo Máx (%)
          </label>
          <input
            type="range"
            min="0.10"
            max="0.50"
            step="0.05"
            value={localProtection.maxCapitalAtRiskPct}
            onChange={(e) =>
              handleChange("maxCapitalAtRiskPct", parseFloat(e.target.value))
            }
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-500 mt-1">
            <span>10%</span>
            <span>{safeMultiply(localProtection.maxCapitalAtRiskPct, 100).toFixed(0)}%</span>
            <span>50%</span>
          </div>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Stop Loss Automático (%)
          </label>
          <input
            type="range"
            min="0.05"
            max="0.30"
            step="0.01"
            value={localProtection.autoStopLossPct}
            onChange={(e) =>
              handleChange("autoStopLossPct", parseFloat(e.target.value))
            }
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-500 mt-1">
            <span>5%</span>
            <span>{safeMultiply(localProtection.autoStopLossPct, 100).toFixed(0)}%</span>
            <span>30%</span>
          </div>
        </div>

        <div>
          <label className="block text-sm text-gray-400 mb-1">
            Apalancamiento Máximo
          </label>
          <input
            type="range"
            min="1"
            max="50"
            step="1"
            value={localProtection.maxLeverage}
            onChange={(e) =>
              handleChange("maxLeverage", parseInt(e.target.value))
            }
            className="w-full"
          />
          <div className="flex justify-between text-xs text-gray-500 mt-1">
            <span>1x</span>
            <span className="text-primary-400">{localProtection.maxLeverage}x</span>
            <span>50x</span>
          </div>
        </div>

        <div className="flex items-center justify-between pt-2">
          <div>
            <span className="text-sm text-gray-400">Stop Loss Automático</span>
            <p className="text-xs text-gray-500">
              Activa órdenes de venta cuando se alcance el umbral
            </p>
          </div>
          <button
            onClick={() =>
              handleChange("autoStopLossEnabled", !localProtection.autoStopLossEnabled)
            }
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              localProtection.autoStopLossEnabled ? "bg-success" : "bg-dark-300"
            }`}
          >
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                localProtection.autoStopLossEnabled ? "translate-x-6" : "translate-x-1"
              }`}
            />
          </button>
        </div>

        <div className="flex items-center justify-between pt-2 border-t border-dark-300">
          <div>
            <span className="text-sm text-gray-400">Emergencia - Auto Retirada</span>
            <p className="text-xs text-gray-500">
              Intenta retirar fondos si se activa el kill switch
            </p>
          </div>
          <button
            onClick={() =>
              handleChange(
                "emergencyWithdrawEnabled",
                !localProtection.emergencyWithdrawEnabled
              )
            }
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              localProtection.emergencyWithdrawEnabled
                ? "bg-danger"
                : "bg-dark-300"
            }`}
          >
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                localProtection.emergencyWithdrawEnabled
                  ? "translate-x-6"
                  : "translate-x-1"
              }`}
            />
          </button>
        </div>
      </div>

      <div className="mt-4 p-3 bg-warning/10 border border-warning/20 rounded-lg">
        <div className="flex items-start gap-2">
          <AlertTriangle className="w-4 h-4 text-warning mt-0.5" />
          <div className="text-xs text-warning">
            <p className="font-medium">Validaciones activas:</p>
            <ul className="list-disc list-inside mt-1 space-y-0.5">
              <li>Límite de posición apalancada</li>
              <li>Capital en riesgo máximo</li>
              <li>Stop loss automático</li>
              <li>Kill switch por balance crítico</li>
            </ul>
          </div>
        </div>
      </div>

      {isEditing && (
        <div className="flex gap-2 pt-4 border-t border-dark-300 mt-4">
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
  );
};