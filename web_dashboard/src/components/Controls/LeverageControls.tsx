import React, { useState } from "react";
import { useConfigStore } from "../../store/configStore";
import { Save, RotateCcw, TrendingUp } from "lucide-react";
import toast from "react-hot-toast";

const EXCHANGES = [
  { value: "simulated", label: "Simulado" },
  { value: "hyperliquid_mainnet", label: "Hyperliquid" },
  { value: "bybit", label: "Bybit" },
];

const SYMBOLS = ["BTC", "ETH", "BNB", "SOL", "XRP"];

export const LeverageControls: React.FC = () => {
  const { config, updateLeverage } = useConfigStore();
  const [selectedExchange, setSelectedExchange] = useState("simulated");
  const [localLeverage, setLocalLeverage] = useState<Map<string, number>>(new Map());
  const [isEditing, setIsEditing] = useState(false);

  const exchangeLeverage = config.leverage.filter(
    (l) => l.exchange === selectedExchange
  );

  const handleLeverageChange = (symbol: string, value: number) => {
    setLocalLeverage((prev) => new Map(prev).set(symbol, value));
    setIsEditing(true);
  };

  const handleSave = async () => {
    try {
      const updatedLeverage = config.leverage.map((l) => {
        if (l.exchange === selectedExchange) {
          const newLev = localLeverage.get(l.symbol);
          return newLev !== undefined
            ? { ...l, leverage: newLev }
            : l;
        }
        return l;
      });
      await updateLeverage(updatedLeverage);
      setIsEditing(false);
      toast.success("Apalancamiento actualizado");
    } catch (error) {
      toast.error("Error al guardar");
    }
  };

  const handleReset = () => {
    setLocalLeverage(new Map());
    setIsEditing(false);
  };

  const getLeverageValue = (symbol: string): number => {
    if (localLeverage.has(symbol)) {
      return localLeverage.get(symbol)!;
    }
    const existing = exchangeLeverage.find((l) => l.symbol === symbol);
    return existing?.leverage ?? 1;
  };

  const maxLeverage = config.protection.maxLeverage || 10;

  return (
    <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold">Apalancamiento por Símbolo</h3>
        <TrendingUp className="w-5 h-5 text-primary-500" />
      </div>

      <div className="mb-4">
        <label className="block text-sm text-gray-400 mb-2">Exchange</label>
        <select
          value={selectedExchange}
          onChange={(e) => setSelectedExchange(e.target.value)}
          className="w-full bg-dark-300 border border-dark-400 rounded-lg px-3 py-2 text-white"
        >
          {EXCHANGES.map((ex) => (
            <option key={ex.value} value={ex.value}>
              {ex.label}
            </option>
          ))}
        </select>
      </div>

      <div className="space-y-4">
        {SYMBOLS.map((symbol) => {
          const currentLeverage = getLeverageValue(symbol);
          return (
            <div key={symbol} className="flex items-center justify-between">
              <span className="text-sm font-medium text-gray-300 w-16">
                {symbol}
              </span>
              <div className="flex-1 mx-4">
                <input
                  type="range"
                  min={1}
                  max={maxLeverage}
                  step={1}
                  value={currentLeverage}
                  onChange={(e) =>
                    handleLeverageChange(symbol, parseInt(e.target.value))
                  }
                  className="w-full"
                />
              </div>
              <span className="text-sm font-bold text-primary-400 w-12 text-right">
                {currentLeverage}x
              </span>
            </div>
          );
        })}
      </div>

      <div className="mt-4 p-3 bg-dark-300 rounded-lg">
        <div className="flex justify-between text-sm">
          <span className="text-gray-400">Apalancamiento máximo global:</span>
          <span className="text-warning font-medium">{maxLeverage}x</span>
        </div>
      </div>

      {isEditing && (
        <div className="flex gap-2 pt-4 border-t border-dark-300 mt-4">
          <button
            onClick={handleSave}
            className="flex-1 flex items-center justify-center gap-2 bg-primary-600 hover:bg-primary-700 text-white rounded-lg px-4 py-2 text-sm transition-colors"
          >
            <Save className="w-4 h-4" />
            Guardar
          </button>
          <button
            onClick={handleReset}
            className="flex items-center justify-center gap-2 bg-dark-300 hover:bg-dark-400 text-gray-300 rounded-lg px-4 py-2 text-sm transition-colors"
          >
            <RotateCcw className="w-4 h-4" />
          </button>
        </div>
      )}
    </div>
  );
};