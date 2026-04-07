import React from "react";
import { useBotStore } from "../../store/botStore";
import { TrendingUp, TrendingDown } from "lucide-react";

export const PositionsTable: React.FC = () => {
  const { stats } = useBotStore();

  if (!stats) {
    return (
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
        <h3 className="text-lg font-semibold mb-4">Posiciones Abiertas</h3>
        <div className="text-center py-8 text-gray-400">
          Cargando datos...
        </div>
      </div>
    );
  }

  const positions = stats.positions || [];

  if (positions.length === 0) {
    return (
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
        <h3 className="text-lg font-semibold mb-4">Posiciones Abiertas</h3>
        <div className="text-center py-8 text-gray-400">
          No hay posiciones abiertas
        </div>
      </div>
    );
  }

  return (
    <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
      <h3 className="text-lg font-semibold mb-4">Posiciones Abiertas</h3>
      <div className="overflow-x-auto">
        <table className="w-full">
          <thead>
            <tr className="text-left text-sm text-gray-400 border-b border-dark-300">
              <th className="pb-3">Símbolo</th>
              <th className="pb-3">Tamaño</th>
              <th className="pb-3">Entrada</th>
              <th className="pb-3">Actual</th>
              <th className="pb-3">P&L No Realizado</th>
              <th className="pb-3">Valor</th>
            </tr>
          </thead>
          <tbody className="font-mono text-sm">
            {positions.map((pos) => {
              const size = pos.size || 0;
              const entryPrice = pos.entryPrice || 0;
              const currentPrice = pos.currentPrice || 0;
              const unrealizedPnl = pos.unrealizedPnl || 0;
              const isLong = size > 0;
              const totalValue = Math.abs(size) * currentPrice;

              return (
                <tr key={pos.symbol} className="border-b border-dark-300">
                  <td className="py-3">
                    <div className="flex items-center gap-2">
                      {isLong ? (
                        <TrendingUp className="w-4 h-4 text-success" />
                      ) : (
                        <TrendingDown className="w-4 h-4 text-danger" />
                      )}
                      <span>{pos.symbol}</span>
                    </div>
                  </td>
                  <td className="py-3">{Math.abs(size).toFixed(4)}</td>
                  <td className="py-3">${entryPrice.toFixed(2)}</td>
                  <td className="py-3">${currentPrice.toFixed(2)}</td>
                  <td className={unrealizedPnl >= 0 ? "text-success" : "text-danger"}>
                    ${unrealizedPnl.toFixed(2)}
                  </td>
                  <td className="py-3">${totalValue.toFixed(2)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
};