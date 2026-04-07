import { useBotStore } from "../store/botStore";
import { WebSocketMessage } from "./types";
import toast from "react-hot-toast";

// Usar la URL absoluta
const WS_URL = 'ws://localhost:3001/ws';

class WebSocketService {
  private ws: WebSocket | null = null;
  private reconnectAttempts = 0;
  private maxReconnectAttempts = 10;
  private reconnectTimeout = 1000;
  private listeners: Map<string, Set<(data: any) => void>> = new Map();
  private pingInterval: number | null = null;

  connect() {
    console.log("🔌 WebSocketService.connect() llamado a:", WS_URL);
    this.reconnectAttempts = 0;
    this.createConnection();
  }

  private createConnection() {
    console.log("🔌 Intentando conectar a WebSocket:", WS_URL);
    
    try {
      this.ws = new WebSocket(WS_URL);
      
      this.ws.onopen = () => {
        console.log("✅ WebSocket CONECTADO exitosamente");
        this.reconnectAttempts = 0;
        
        useBotStore.getState().setStatus({
          running: false,
          mode: "paper",
          connected: true,
          uptime: 0,
        });
        
        toast.success("Conectado al bot");
        
        this.pingInterval = window.setInterval(() => {
          if (this.ws?.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify({ type: "ping" }));
          }
        }, 15000);
      };

      this.ws.onmessage = (event) => {
        try {
          const message: WebSocketMessage = JSON.parse(event.data);
          
          if (message.type === "stats") {
            const mappedStats = {
              balance: message.data.balance || 0,
              pnl: message.data.pnl || 0,
              pnlPercent: message.data.pnl_percent || 0,
              dailyPnl: message.data.daily_pnl || 0,
              drawdown: message.data.drawdown || 0,
              winRate: message.data.win_rate || 0,
              totalTrades: message.data.total_trades || 0,
              winningTrades: message.data.winning_trades || 0,
              losingTrades: message.data.losing_trades || 0,
              volume: message.data.volume || 0,
              positions: message.data.positions || [],
              trades: message.data.trades || [],
              history: message.data.history || [],
            };
            
            useBotStore.getState().setStats(mappedStats);
            
          } else if (message.type === "alert") {
            const listeners = this.listeners.get("alert");
            if (listeners) {
              listeners.forEach((callback) => callback(message.data));
            }
          }
        } catch (error) {
          console.error("❌ Error parseando mensaje:", error);
        }
      };

      this.ws.onclose = () => {
        console.log("🔌 WebSocket CERRADO");
        
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
      this.ws.send(JSON.stringify({ type, data }));
    }
  }

  public subscribe(type: string, callback: (data: any) => void) {
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