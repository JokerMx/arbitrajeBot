import { useBotStore } from "../store/botStore";
import { WebSocketMessage } from "./types";
import toast from "react-hot-toast";

class WebSocketService {
  private ws: WebSocket | null = null;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 10;
  private reconnectTimeout = 1000;
  private listeners: Map<string, Set<(data: any) => void>> = new Map();
  private url: string = "";
  private pingInterval: number | null = null;

  connect(url: string) {
    console.log("🔌 WebSocketService.connect() llamado con URL:", url);
    this.url = url;
    this.reconnectAttempts = 0;
    this.createConnection();
  }

  private createConnection() {
    console.log("🔌 Intentando conectar a WebSocket:", this.url);
    
    try {
      this.ws = new WebSocket(this.url);
      
      this.ws.onopen = () => {
        console.log("✅ WebSocket CONECTADO exitosamente a", this.url);
        this.reconnectAttempts = 0;
        
        useBotStore.getState().setStatus({
          running: false,
          mode: "paper",
          connected: true,
          uptime: 0,
        });
        
        toast.success("Conectado al bot");
        console.log("📊 Enviando ping inicial...");
        
        this.pingInterval = window.setInterval(() => {
          if (this.ws?.readyState === WebSocket.OPEN) {
            console.log("📤 Enviando ping...");
            this.ws.send(JSON.stringify({ type: "ping" }));
          } else {
            console.warn("⚠️ WebSocket no está abierto, no se puede enviar ping");
          }
        }, 15000);
      };

      this.ws.onmessage = (event) => {
        console.log("📥 Mensaje RECIBIDO del WebSocket:", event.data.substring(0, 150) + "...");
        
        try {
          const message: WebSocketMessage = JSON.parse(event.data);
          console.log("📦 Mensaje parseado:", message.type);
          
          if (message.type === "stats") {
            console.log("📊 Stats RECIBIDOS del servidor (raw):", message.data);
            
            // Verificar qué campos vienen del servidor
            console.log("   Campos en raw:", Object.keys(message.data));
            console.log("   win_rate:", message.data.win_rate);
            console.log("   winning_trades:", message.data.winning_trades);
            console.log("   losing_trades:", message.data.losing_trades);
            console.log("   total_trades:", message.data.total_trades);
            
            // MAPEO: Convertir de snake_case (backend) a camelCase (frontend)
            const mappedStats = {
              balance: message.data.balance || 0,
              pnl: message.data.pnl || 0,
              pnlPercent: message.data.pnl_percent || 0,
              dailyPnl: message.data.daily_pnl || 0,
              drawdown: message.data.drawdown || 0,
              winRate: message.data.win_rate || 0,                    // snake_case → camelCase
              totalTrades: message.data.total_trades || 0,
              winningTrades: message.data.winning_trades || 0,        // snake_case → camelCase
              losingTrades: message.data.losing_trades || 0,          // snake_case → camelCase
              volume: message.data.volume || 0,
              positions: message.data.positions || [],
              trades: message.data.trades || [],
              history: message.data.history || [],
            };
            
            console.log("📊 Stats mapeados a camelCase:", mappedStats);
            console.log("   winRate (mapeado):", mappedStats.winRate);
            console.log("   winningTrades (mapeado):", mappedStats.winningTrades);
            console.log("   losingTrades (mapeado):", mappedStats.losingTrades);
            
            // Actualizar el store
            useBotStore.getState().setStats(mappedStats);
            console.log("✅ Stats actualizados en el store");
            
          } else if (message.type === "alert") {
            console.log("⚠️ Alerta recibida:", message.data);
            const listeners = this.listeners.get("alert");
            if (listeners) {
              listeners.forEach((callback) => callback(message.data));
            }
          } else {
            console.log("❓ Tipo de mensaje desconocido:", message.type, message);
          }
        } catch (error) {
          console.error("❌ Error parseando mensaje:", error);
          console.error("   Data original:", event.data);
        }
      };

      this.ws.onclose = (event) => {
        console.log("🔌 WebSocket CERRADO. Código:", event.code, "Razón:", event.reason);
        
        if (this.pingInterval) {
          clearInterval(this.pingInterval);
          this.pingInterval = null;
        }
        
        useBotStore.getState().setStatus({
          running: false,
          mode: "paper",
          connected: false,
          uptime: 0,
        });
        
        console.log("🔄 Intentando reconectar...");
        this.reconnect();
      };

      this.ws.onerror = (error) => {
        console.error("❌ WebSocket ERROR:", error);
      };
      
    } catch (error) {
      console.error("❌ Error CRÍTICO creando WebSocket:", error);
      this.reconnect();
    }
  }

  private reconnect() {
    if (this.reconnectAttempts >= this.maxReconnectAttempts) {
      console.error("❌ No se pudo reconectar después de", this.maxReconnectAttempts, "intentos");
      toast.error("No se pudo reconectar. Recarga la página.");
      return;
    }

    this.reconnectAttempts++;
    const timeout = this.reconnectTimeout * Math.pow(1.5, this.reconnectAttempts - 1);
    
    console.log(`🔄 RECONECTANDO en ${timeout}ms... (intento ${this.reconnectAttempts}/${this.maxReconnectAttempts})`);
    setTimeout(() => {
      this.createConnection();
    }, timeout);
  }

  public send(type: string, data: any) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      console.log("📤 Enviando mensaje:", type);
      this.ws.send(JSON.stringify({ type, data }));
    } else {
      console.warn("⚠️ WebSocket no conectado, mensaje no enviado:", type);
    }
  }

  public subscribe(type: string, callback: (data: any) => void) {
    console.log("📻 Suscribiendo a tipo:", type);
    if (!this.listeners.has(type)) {
      this.listeners.set(type, new Set());
    }
    this.listeners.get(type)!.add(callback);
  }

  public unsubscribe(type: string, callback: (data: any) => void) {
    const listeners = this.listeners.get(type);
    if (listeners) {
      listeners.delete(callback);
    }
  }

  public disconnect() {
    console.log("🔌 Desconectando WebSocket manualmente...");
    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = null;
    }
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
    this.listeners.clear();
  }
}

export const wsService = new WebSocketService();