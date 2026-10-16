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
      <Header onNavigate={onNavigate} />
      <Sidebar onNavigate={onNavigate} currentView={currentView}>
        {children}
      </Sidebar>
    </div>
  );
};