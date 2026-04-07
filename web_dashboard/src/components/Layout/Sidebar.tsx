import React, { useState, useEffect } from "react";
import { 
  LayoutDashboard, 
  LineChart, 
  Settings, 
  AlertCircle,
  TrendingUp,
  Shield,
  Coins,
  ChevronLeft,
  ChevronRight,
  Menu,
  X,
  Bell,
  BarChart3,
  History,
  Users
} from "lucide-react";
import { useBotStore } from "../../store/botStore";

interface SidebarItemProps {
  icon: React.ElementType;
  label: string;
  view: string;
  currentView: string;
  collapsed?: boolean;
  onClick: (view: string) => void;
}

const SidebarItem: React.FC<SidebarItemProps> = ({ 
  icon: Icon, 
  label, 
  view,
  currentView,
  collapsed,
  onClick 
}) => {
  const isActive = currentView === view;
  
  return (
    <button
      onClick={() => onClick(view)}
      className={`w-full flex items-center gap-3 px-3 py-3 rounded-lg transition-all duration-200 ${
        isActive 
          ? "bg-primary-600 text-white shadow-lg shadow-primary-600/20" 
          : "text-gray-400 hover:bg-dark-300 hover:text-white"
      } ${collapsed ? 'justify-center' : 'justify-start'}`}
      title={collapsed ? label : undefined}
    >
      <Icon className={`w-5 h-5 ${isActive ? 'animate-pulse' : ''}`} />
      {!collapsed && (
        <span className="text-sm font-medium whitespace-nowrap">{label}</span>
      )}
      {isActive && !collapsed && (
        <span className="ml-auto w-1.5 h-1.5 bg-white rounded-full animate-pulse"></span>
      )}
    </button>
  );
};

interface SidebarProps {
  onNavigate: (view: string) => void;
  currentView: string;
}

export const Sidebar: React.FC<SidebarProps> = ({ onNavigate, currentView }) => {
  const [collapsed, setCollapsed] = useState(false);
  const [mobileOpen, setMobileOpen] = useState(false);
  const [isMobile, setIsMobile] = useState(false);
  const { stats } = useBotStore();
  
  const balance = stats?.balance ?? 0;
  const pnlPercent = stats?.pnlPercent ?? 0;
  const pnlColor = pnlPercent >= 0 ? "text-success" : "text-danger";
  const pnlPrefix = pnlPercent >= 0 ? "+" : "";

  // Detectar si es móvil
  useEffect(() => {
    const checkMobile = () => {
      setIsMobile(window.innerWidth < 768);
      if (window.innerWidth < 768) {
        setCollapsed(true);
      }
    };
    
    checkMobile();
    window.addEventListener('resize', checkMobile);
    return () => window.removeEventListener('resize', checkMobile);
  }, []);

  const handleItemClick = (view: string) => {
    console.log("🖱️ Click en item:", view);
    onNavigate(view);
    if (isMobile) {
      setMobileOpen(false);
    }
  };

  const toggleCollapse = () => {
    setCollapsed(!collapsed);
  };

  const toggleMobile = () => {
    setMobileOpen(!mobileOpen);
  };

  // Lista de items del sidebar
  const menuItems = [
    { icon: LayoutDashboard, label: "Dashboard", view: "dashboard" },
    { icon: BarChart3, label: "Estadísticas", view: "stats" },
    { icon: TrendingUp, label: "Estrategia", view: "strategy" },
    { icon: Shield, label: "Riesgo", view: "risk" },
    { icon: Coins, label: "Mercados", view: "markets" },
    { icon: History, label: "Historial", view: "history" },
    { icon: Bell, label: "Alertas", view: "alerts" },
    { icon: Users, label: "Usuarios", view: "users" },
    { icon: Settings, label: "Configuración", view: "settings" },
  ];

  // Versión móvil con menú hamburguesa
  if (isMobile) {
    return (
      <>
        {/* Botón de menú móvil */}
        <button
          onClick={toggleMobile}
          className="fixed top-4 left-4 z-50 p-2 bg-dark-200 rounded-lg border border-dark-300 shadow-lg"
        >
          {mobileOpen ? <X className="w-5 h-5" /> : <Menu className="w-5 h-5" />}
        </button>

        {/* Overlay */}
        {mobileOpen && (
          <div 
            className="fixed inset-0 bg-black/50 backdrop-blur-sm z-40"
            onClick={toggleMobile}
          />
        )}

        {/* Sidebar móvil */}
        <aside className={`fixed left-0 top-0 h-full w-64 bg-dark-200 border-r border-dark-300 p-4 z-50 transform transition-transform duration-300 ease-in-out ${
          mobileOpen ? 'translate-x-0' : '-translate-x-full'
        }`}>
          <div className="flex items-center justify-between mb-8 mt-4">
            <div className="flex items-center gap-2">
              <div className="w-8 h-8 bg-primary-600 rounded-lg flex items-center justify-center">
                <span className="text-white font-bold">H</span>
              </div>
              <h2 className="font-bold text-lg">Hyperliquid</h2>
            </div>
            <button onClick={toggleMobile} className="p-1 hover:bg-dark-300 rounded-lg">
              <X className="w-5 h-5" />
            </button>
          </div>

          <div className="space-y-2">
            {menuItems.map((item) => (
              <SidebarItem
                key={item.view}
                icon={item.icon}
                label={item.label}
                view={item.view}
                currentView={currentView}
                onClick={handleItemClick}
              />
            ))}
          </div>

          <div className="absolute bottom-4 left-4 right-4">
            <div className="p-3 bg-dark-300 rounded-lg border border-dark-400">
              <p className="text-xs text-gray-400">Balance</p>
              <p className={`text-sm font-bold ${pnlColor}`}>${balance.toFixed(2)}</p>
              <p className={`text-xs text-gray-500 mt-1 ${pnlColor}`}>{pnlPrefix}${pnlPercent.toFixed(2)}%</p>
            </div>
          </div>
        </aside>
      </>
    );
  }

  // Versión desktop con sidebar contraíble
  return (
    <>
      <aside className={`fixed left-0 top-16 h-[calc(100vh-4rem)] bg-dark-200 border-r border-dark-300 transition-all duration-300 ${
        collapsed ? 'w-20' : 'w-64'
      }`}>
        {/* Botón de colapsar */}
        <button
          onClick={toggleCollapse}
          className="absolute -right-3 top-8 w-6 h-6 bg-dark-300 border border-dark-400 rounded-full flex items-center justify-center hover:bg-dark-400 transition-colors z-10"
        >
          {collapsed ? (
            <ChevronRight className="w-4 h-4" />
          ) : (
            <ChevronLeft className="w-4 h-4" />
          )}
        </button>

        <div className="p-4 h-full overflow-y-auto scrollbar-thin scrollbar-thumb-dark-400 scrollbar-track-dark-300">
          <div className="space-y-2">
            {menuItems.map((item) => (
              <SidebarItem
                key={item.view}
                icon={item.icon}
                label={item.label}
                view={item.view}
                currentView={currentView}
                collapsed={collapsed}
                onClick={handleItemClick}
              />
            ))}
          </div>

          {!collapsed && (
            <div className="mt-8 p-4 bg-dark-300/50 rounded-lg border border-dark-400">
              <p className="text-xs text-gray-400 mb-1">Balance</p>
              <p className={`text-lg font-bold ${pnlColor}`}>${balance.toFixed(2)}</p>
              <div className="flex items-center justify-between mt-2">
                <span className="text-xs text-gray-400">P&L</span>
                <span className={`text-xs ${pnlColor}`}>{pnlPrefix}{pnlPercent.toFixed(2)}%</span>
              </div>
              <div className="w-full h-1 bg-dark-400 rounded-full mt-2">
                <div 
                  className={`h-1 rounded-full ${pnlPercent >= 0 ? 'bg-success' : 'bg-danger'}`} 
                  style={{ width: `${Math.min(Math.abs(pnlPercent), 100)}%` }}
                ></div>
              </div>
            </div>
          )}
        </div>
      </aside>

      {/* Ajuste del margen del contenido principal */}
      <style>{`
        main {
          margin-left: ${collapsed ? '80px' : '256px'};
          transition: margin-left 0.3s;
        }
        @media (max-width: 768px) {
          main {
            margin-left: 0;
          }
        }
      `}</style>
    </>
  );
};