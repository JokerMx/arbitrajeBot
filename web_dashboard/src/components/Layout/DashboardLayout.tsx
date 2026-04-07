import React from "react";
import { Sidebar } from "./Sidebar";
import { Header } from "./Header";

interface DashboardLayoutProps {
  children: React.ReactNode;
  onNavigate: (view: string) => void;
  currentView: string;
}

export const DashboardLayout: React.FC<DashboardLayoutProps> = ({ 
  children, 
  onNavigate, 
  currentView 
}) => {
  return (
    <div className="min-h-screen bg-dark-100 text-white">
      <Header />
      <Sidebar onNavigate={onNavigate} currentView={currentView} />
      <main className="flex-1 p-6 transition-all duration-300 md:ml-64 lg:ml-64">
        <div className="max-w-7xl mx-auto">
          {children}
        </div>
      </main>
    </div>
  );
};