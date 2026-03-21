import React from "react";
import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
} from "recharts";
import { useBotStore } from "../../store/botStore";
import { format, parseISO, differenceInMinutes } from "date-fns";
import { es } from "date-fns/locale";

interface HistoryPoint {
  timestamp: string;
  balance: number;
  pnl: number;
}

interface FiveMinMark {
  value: string;
  label: string;
}

// Función para generar marcas cada 5 minutos
const generateFiveMinuteMarks = (data: HistoryPoint[]): FiveMinMark[] => {
  if (!data || data.length === 0) return [];
  
  const marks: FiveMinMark[] = [];
  const firstTimestamp = parseISO(data[0].timestamp);
  const lastTimestamp = parseISO(data[data.length - 1].timestamp);
  
  // Calcular el primer múltiplo de 5 minutos
  const firstDate = new Date(firstTimestamp);
  const minutes = firstDate.getMinutes();
  const remainder = minutes % 5;
  const firstMarkMinutes = remainder === 0 ? minutes : minutes + (5 - remainder);
  firstDate.setMinutes(firstMarkMinutes, 0, 0);
  
  // Generar marcas cada 5 minutos
  let currentMark = new Date(firstDate);
  while (currentMark <= lastTimestamp) {
    // Encontrar el punto de datos más cercano a esta marca
    const closestPoint = data.find(point => {
      const pointTime = parseISO(point.timestamp);
      const diff = Math.abs(differenceInMinutes(pointTime, currentMark));
      return diff < 2.5; // Dentro de 2.5 minutos de la marca
    });
    
    if (closestPoint) {
      marks.push({
        value: closestPoint.timestamp,
        label: format(currentMark, "HH:mm", { locale: es })
      });
    }
    
    // Siguiente marca en 5 minutos
    currentMark = new Date(currentMark.getTime() + 5 * 60000);
  }
  
  return marks;
};

export const BalanceChart: React.FC = () => {
  const { stats } = useBotStore();

  if (!stats?.history || stats.history.length === 0) {
    return (
      <div className="bg-dark-200 rounded-xl p-6 border border-dark-300 h-96">
        <h3 className="text-lg font-semibold mb-4">Evolución del Balance</h3>
        <div className="h-80 flex items-center justify-center text-gray-400">
          No hay datos suficientes
        </div>
      </div>
    );
  }

  // Formatear datos para el gráfico
  const data = stats.history.map((point) => ({
    ...point,
    time: point.timestamp ? 
      format(new Date(point.timestamp), "HH:mm:ss", { locale: es }) : 
      "--:--:--",
    rawTimestamp: point.timestamp,
  }));

  const firstBalance = data[0]?.balance || 0;
  const lastBalance = data[data.length - 1]?.balance || 0;
  const isPositive = lastBalance >= firstBalance;

  // Generar marcas cada 5 minutos
  const fiveMinMarks = generateFiveMinuteMarks(stats.history);
  
  // Crear array de ticks para el eje X (solo strings, sin undefined)
  const ticks: string[] = fiveMinMarks
    .map(m => {
      const point = data.find(d => d.timestamp === m.value);
      return point?.time;
    })
    .filter((time): time is string => time !== undefined);

  return (
    <div className="bg-dark-200 rounded-xl p-6 border border-dark-300">
      <div className="flex items-center justify-between mb-4">
        <h3 className="text-lg font-semibold">Evolución del Balance</h3>
        <div className="flex items-center gap-2 text-xs text-gray-400">
          <span className="w-2 h-2 bg-primary-500 rounded-full animate-pulse"></span>
          <span>Marca cada 5 min</span>
        </div>
      </div>
      
      <div className="h-80">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={data} margin={{ bottom: 20, left: 10, right: 10 }}>
            <defs>
              <linearGradient id="balanceGradient" x1="0" y1="0" x2="0" y2="1">
                <stop
                  offset="5%"
                  stopColor={isPositive ? "#10b981" : "#ef4444"}
                  stopOpacity={0.3}
                />
                <stop
                  offset="95%"
                  stopColor={isPositive ? "#10b981" : "#ef4444"}
                  stopOpacity={0}
                />
              </linearGradient>
            </defs>
            
            <CartesianGrid strokeDasharray="3 3" stroke="#1e293b" />
            
            {/* Eje X con marcas personalizadas - CORREGIDO */}
            <XAxis
              dataKey="time"
              stroke="#64748b"
              tick={{ fill: "#94a3b8", fontSize: 11 }}
              interval="preserveStartEnd"
              ticks={ticks}  // Ahora es string[], no (string | undefined)[]
              tickFormatter={(value: string) => {
                const mark = fiveMinMarks.find(m => {
                  const point = data.find(d => d.time === value);
                  return point?.timestamp === m.value;
                });
                return mark ? mark.label : '';
              }}
            />
            
            <YAxis
              stroke="#64748b"
              tick={{ fill: "#94a3b8", fontSize: 12 }}
              tickFormatter={(value: number) => `$${value.toFixed(2)}`}
              domain={['auto', 'auto']}
            />
            
            <Tooltip
              contentStyle={{
                backgroundColor: "#1e293b",
                border: "1px solid #334155",
                borderRadius: "8px",
                color: "#fff",
              }}
              labelStyle={{ color: "#94a3b8", marginBottom: "4px" }}
              formatter={(value: number) => [`$${value.toFixed(2)}`, "Balance"]}
              labelFormatter={(label: string) => `Hora: ${label}`}
            />
            
            {/* Línea del balance */}
            <Area
              type="monotone"
              dataKey="balance"
              stroke={isPositive ? "#10b981" : "#ef4444"}
              strokeWidth={2}
              fill="url(#balanceGradient)"
              dot={false}
              activeDot={{ r: 6, stroke: "#fff", strokeWidth: 2 }}
            />
            
            {/* Marcadores visuales cada 5 minutos - CORREGIDO */}
            {fiveMinMarks.map((mark) => {
              const point = data.find(d => d.timestamp === mark.value);
              if (!point) return null;
              
              // Encontrar coordenadas del punto (esto es complejo en Recharts)
              // Alternativa: Usar otro Area con dots personalizados
              return null;
            })}
          </AreaChart>
        </ResponsiveContainer>
      </div>

      {/* Leyenda mejorada */}
      <div className="mt-4 flex items-center justify-between text-xs text-gray-400 border-t border-dark-300 pt-3">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-1">
            <span className="w-3 h-3 bg-primary-500 rounded-full animate-pulse"></span>
            <span>Marca cada 5 minutos</span>
          </div>
          <div className="flex items-center gap-1">
            <span className="w-3 h-0.5 bg-success rounded-full"></span>
            <span>Evolución</span>
          </div>
        </div>
        <div>
          {data.length > 0 && (
            <span>
              {format(new Date(data[0].timestamp), "HH:mm")} - {format(new Date(data[data.length-1].timestamp), "HH:mm")}
            </span>
          )}
        </div>
      </div>
    </div>
  );
};