import React from "react";
import { Bell, Settings, Power } from "lucide-react";
import { useBotStore } from "../../store/botStore";

export const Header: React.FC = () => {
  const { status } = useBotStore();

  return (
    <header className="fixed top-0 left-0 right-0 h-16 bg-dark-200 border-b border-dark-300 z-50">
      <div className="flex items-center justify-between h-full px-6">
        <div className="flex items-center gap-3">
          <div className="w-8 h-8 bg-primary-600 rounded-lg flex items-center justify-center">
            <span className="text-white font-bold">H</span>
          </div>
          <h1 className="text-xl font-bold">Hyperliquid Bot</h1>
          <div className="ml-4 flex items-center gap-2">
            <span className={`w-2 h-2 rounded-full ${status?.connected ? 'bg-success animate-pulse' : 'bg-danger'}`} />
            <span className="text-sm text-gray-400">
              {status?.connected ? 'Conectado' : 'Desconectado'}
            </span>
          </div>
        </div>

        <div className="flex items-center gap-4">
          <button className="p-2 hover:bg-dark-300 rounded-lg transition-colors relative">
            <Bell className="w-5 h-5 text-gray-400" />
            <span className="absolute top-1 right-1 w-2 h-2 bg-danger rounded-full"></span>
          </button>
          <button className="p-2 hover:bg-dark-300 rounded-lg transition-colors">
            <Settings className="w-5 h-5 text-gray-400" />
          </button>
          <button className="p-2 hover:bg-dark-300 rounded-lg transition-colors">
            <Power className="w-5 h-5 text-gray-400" />
          </button>
          <div className="w-8 h-8 bg-primary-600 rounded-full flex items-center justify-center ml-2">
            <span className="text-white text-sm font-bold">U</span>
          </div>
        </div>
      </div>
    </header>
  );
};