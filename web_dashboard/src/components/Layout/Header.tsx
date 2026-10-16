import React from "react";
import { Bell, Settings, Power, Pause, Play, RotateCw } from "lucide-react";
import { useBotStore } from "../../store/botStore";
import { useBotStatus, pauseBot, resumeBot, restartBot, formatUptime } from "../../services/botControl";
import { backendApiBaseUrl } from "../../services/backendConfig";

interface HeaderProps {
  onNavigate: (view: string) => void;
}

const SEEN_KEY = "alertsSeenAt";

export const Header: React.FC<HeaderProps> = ({ onNavigate }) => {
  const { status: ws } = useBotStore();
  const { status, online, reload } = useBotStatus();
  const [menuOpen, setMenuOpen] = React.useState(false);
  const [unread, setUnread] = React.useState(0);
  const menuRef = React.useRef<HTMLDivElement>(null);

  // Señales nuevas desde la última vez que se abrió "Alertas"
  React.useEffect(() => {
    const load = async () => {
      try {
        const r = await fetch(`${backendApiBaseUrl}/signals`);
        if (!r.ok) return;
        const recs: { at: string }[] = await r.json();
        let seen = 0;
        try {
          seen = Number(localStorage.getItem(SEEN_KEY) || 0);
        } catch {
          /* sin almacenamiento */
        }
        setUnread(recs.filter((x) => new Date(x.at).getTime() > seen).length);
      } catch {
        /* backend no disponible */
      }
    };
    load();
    const id = setInterval(load, 30000);
    return () => clearInterval(id);
  }, []);

  React.useEffect(() => {
    const close = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setMenuOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, []);

  const openAlerts = () => {
    try {
      localStorage.setItem(SEEN_KEY, String(Date.now()));
    } catch {
      /* sin almacenamiento */
    }
    setUnread(0);
    onNavigate("alerts");
  };

  const act = async (fn: () => Promise<unknown>, confirmMsg?: string) => {
    if (confirmMsg && !window.confirm(confirmMsg)) return;
    setMenuOpen(false);
    await fn();
    setTimeout(reload, 1500);
  };

  const running = online && status && !status.paused;
  const modeLabel = status ? (status.live ? "REAL" : "PAPEL") : "";

  return (
    <header className="fixed top-0 left-0 right-0 h-16 bg-dark-200 border-b border-dark-300 z-50">
      <div className="flex items-center justify-between h-full px-6">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-primary-600 rounded-lg flex items-center justify-center">
            <span className="text-white font-bold">H</span>
          </div>
          <h1 className="text-xl font-bold hidden sm:block">Hyperliquid Bot</h1>
          <div className="ml-4 flex items-center gap-2">
            <span
              className={`w-2 h-2 rounded-full ${
                !online ? "bg-danger" : status?.paused ? "bg-warning" : "bg-success animate-pulse"
              }`}
            />
            <span className="text-sm text-gray-300">
              {!online ? "Bot sin conexión" : status?.paused ? "En pausa" : "Operando"}
            </span>
            {status && (
              <span
                className={`text-[11px] px-2 py-0.5 rounded-full font-semibold ${
                  status.live ? "bg-danger/20 text-danger" : "bg-warning/15 text-warning"
                }`}
                title={status.live ? "Opera con dinero real" : "Simulación: no envía órdenes reales"}
              >
                {modeLabel}
              </span>
            )}
            {!ws?.connected && online && (
              <span className="text-[11px] text-gray-500 hidden md:inline">(tiempo real desconectado)</span>
            )}
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={openAlerts}
            className="p-2 hover:bg-dark-300 rounded-lg transition-colors relative"
            title="Alertas y señales"
          >
            <Bell className="w-5 h-5 text-gray-400" />
            {unread > 0 && (
              <span className="absolute -top-0.5 -right-0.5 min-w-[18px] h-[18px] px-1 bg-danger rounded-full text-[10px] font-bold flex items-center justify-center">
                {unread > 99 ? "99+" : unread}
              </span>
            )}
          </button>
          <button
            onClick={() => onNavigate("settings")}
            className="p-2 hover:bg-dark-300 rounded-lg transition-colors"
            title="Configuración"
          >
            <Settings className="w-5 h-5 text-gray-400" />
          </button>
          <div className="relative" ref={menuRef}>
            <button
              onClick={() => setMenuOpen((v) => !v)}
              className={`p-2 hover:bg-dark-300 rounded-lg transition-colors ${
                running ? "text-success" : status?.paused ? "text-warning" : "text-gray-400"
              }`}
              title="Control del bot"
            >
              <Power className="w-5 h-5" />
            </button>
            {menuOpen && (
              <div className="absolute right-0 mt-2 w-72 bg-dark-200 border border-dark-300 rounded-xl shadow-xl p-2 text-sm">
                <div className="px-3 py-2 text-xs text-gray-400 border-b border-dark-300 mb-1">
                  {status ? (
                    <>
                      Estrategia: <span className="text-gray-200">{status.strategy_mode === "trend" ? "tendencia + rebote" : "market making"}</span>
                      <br />
                      Activo hace {formatUptime(status.uptime_secs)} · v{status.version}
                      <br />
                      Riesgo: {status.risk.status}
                      {status.risk.kill_switch && <span className="text-danger"> · kill switch activado</span>}
                    </>
                  ) : (
                    "Sin conexión con el bot"
                  )}
                </div>
                {status?.can_pause &&
                  (status.paused ? (
                    <button
                      onClick={() => act(resumeBot)}
                      className="w-full flex items-center gap-2 px-3 py-2 rounded-lg hover:bg-dark-300 text-success"
                    >
                      <Play className="w-4 h-4" /> Reanudar entradas
                    </button>
                  ) : (
                    <button
                      onClick={() =>
                        act(
                          pauseBot,
                          "¿Pausar el bot? No abrirá nuevas posiciones; las abiertas se siguen gestionando con su stop."
                        )
                      }
                      className="w-full flex items-center gap-2 px-3 py-2 rounded-lg hover:bg-dark-300 text-warning"
                    >
                      <Pause className="w-4 h-4" /> Pausar nuevas entradas
                    </button>
                  ))}
                <button
                  onClick={() =>
                    act(
                      restartBot,
                      "¿Reiniciar el bot? Vuelve en ~30 segundos. Necesario tras cambiar el capital inicial o Telegram."
                    )
                  }
                  className="w-full flex items-center gap-2 px-3 py-2 rounded-lg hover:bg-dark-300 text-gray-200"
                >
                  <RotateCw className="w-4 h-4" /> Reiniciar bot
                </button>
              </div>
            )}
          </div>
        </div>
      </div>
    </header>
  );
};
