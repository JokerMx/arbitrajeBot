import React from "react";
import { useBotStore } from "../../store/botStore";
import { format } from "date-fns";
import { es } from "date-fns/locale";
import { TrendingUp, TrendingDown } from "lucide-react";

export const TradesTable: React.FC = () => {
  const { stats } = useBotStore();

  if (!stats) {
    return (
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
        <h3 className="text-lg font-semibold mb-4">Historial de Trades</h3>
        <div className="text-center py-8 text-gray-400">
          Cargando datos...
        </div>
      </div>
    );
  }

  // Asegurar que stats.trades existe
  const trades = stats.trades || [];
  const winningTrades = stats.winningTrades || 0;
  const losingTrades = stats.losingTrades || 0;
  const totalTrades = winningTrades + losingTrades;

  return (
    <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
      <h3 className="text-lg font-semibold mb-4">Historial de Trades</h3>
      
      {/* Resumen de trades */}
      <div className="grid grid-cols-3 gap-4 mb-6 text-center">
        <div className="bg-dark-300/50 rounded-lg p-3">
          <p className="text-gray-400 text-sm mb-1">Totales</p>
          <p className="text-2xl font-bold text-white">{totalTrades}</p>
        </div>
        <div className="bg-success/10 rounded-lg p-3">
          <p className="text-success text-sm mb-1">Ganadores</p>
          <p className="text-2xl font-bold text-success">{winningTrades}</p>
        </div>
        <div className="bg-danger/10 rounded-lg p-3">
          <p className="text-danger text-sm mb-1">Perdedores</p>
          <p className="text-2xl font-bold text-danger">{losingTrades}</p>
        </div>
      </div>

      {/* Lista de trades */}
      {trades.length > 0 ? (
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead>
              <tr className="text-left text-sm text-gray-400 border-b border-dark-300">
                <th className="pb-3">Hora</th>
                <th className="pb-3">Símbolo</th>
                <th className="pb-3">Tipo</th>
                <th className="pb-3">Cantidad</th>
                <th className="pb-3">Precio</th>
                <th className="pb-3">Valor</th>
                <th className="pb-3">P&L</th>
              </tr>
            </thead>
            <tbody className="font-mono text-sm">
              {trades.slice(0, 10).map((trade) => (
                <tr key={trade.id} className="border-b border-dark-300 hover:bg-dark-300/50">
                  <td className="py-3">
                    {trade.timestamp ? 
                      format(new Date(trade.timestamp), "HH:mm:ss", { locale: es }) : 
                      "--:--:--"
                    }
                  </td>
                  <td className="py-3 font-semibold">{trade.symbol || "---"}</td>
                  <td className="py-3">
                    <div className="flex items-center gap-2">
                      {trade.side === "COMPRA" ? (
                        <TrendingUp className="w-4 h-4 text-success" />
                      ) : (
                        <TrendingDown className="w-4 h-4 text-danger" />
                      )}
                      <span
                        className={
                          trade.side === "COMPRA" ? "text-success" : "text-danger"
                        }
                      >
                        {trade.side || "---"}
                      </span>
                    </div>
                  </td>
                  <td className="py-3">{(trade.size || 0).toFixed(6)}</td>
                  <td className="py-3">${(trade.price || 0).toFixed(2)}</td>
                  <td className="py-3">${(trade.value || 0).toFixed(2)}</td>
                  <td
                    className={`font-bold ${
                      (trade.pnl || 0) > 0
                        ? "text-success"
                        : (trade.pnl || 0) < 0
                        ? "text-danger"
                        : "text-gray-400"
                    }`}
                  >
                    {(trade.pnl || 0) > 0 ? "+" : ""}{(trade.pnl || 0).toFixed(2)} USD
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="text-center py-8 text-gray-400">
          No hay trades en el historial
        </div>
      )}
    </div>
  );
};