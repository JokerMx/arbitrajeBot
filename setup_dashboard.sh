#!/bin/bash

echo "🚀 Configurando dashboard para Hyperliquid Bot..."

# Ir al directorio del dashboard
cd /home/jokermx/Codigos/botArbitraje/web_dashboard

# Verificar Node.js
if ! command -v node &> /dev/null; then
    echo "📦 Node.js no encontrado. Instalando..."
    curl -fsSL https://deb.nodesource.com/setup_18.x | sudo -E bash -
    sudo apt-get install -y nodejs
fi

# Verificar npm
if ! command -v npm &> /dev/null; then
    echo "📦 npm no encontrado. Instalando..."
    sudo apt-get install -y npm
fi

# Instalar TypeScript globalmente
echo "📦 Instalando TypeScript..."
sudo npm install -g typescript

# Instalar dependencias del dashboard
echo "📦 Instalando dependencias del dashboard..."
npm install

# Construir dashboard
echo "🏗️  Construyendo dashboard..."
npm run build

echo "✅ Dashboard construido correctamente en ./dist"
echo "🚀 Para ejecutar el bot completo: cd .. && cargo run"