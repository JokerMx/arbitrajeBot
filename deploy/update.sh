#!/bin/bash
# Actualiza y reinicia el bot en el servidor.
# Uso: ./deploy/update.sh   (desde la raíz del proyecto, con el usuario dueño de /opt/arbitrajeBot)
set -euo pipefail

cd "$(dirname "$0")/.."

echo "💾 Respaldando datos..."
./deploy/backup.sh

echo "📥 Descargando cambios..."
git pull --ff-only

echo "🦀 Compilando bot (Rust >= 1.91)..."
cargo build --release --locked

echo "🧪 Ejecutando tests..."
cargo test --release --locked

echo "🖥️  Compilando dashboard..."
(cd web_dashboard && npm ci && npm run build)

echo "🔄 Reiniciando servicio..."
sudo systemctl restart arbitrajebot
sleep 5
sudo systemctl --no-pager --lines=20 status arbitrajebot
