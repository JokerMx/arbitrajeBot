#!/bin/bash

echo "🔧 Corrigiendo nombres de campos en server.rs..."

# Reemplazar win_rate por winRate
sed -i 's/win_rate/winRate/g' src/api/server.rs

# Reemplazar winning_trades por winningTrades
sed -i 's/winning_trades/winningTrades/g' src/api/server.rs

# Reemplazar losing_trades por losingTrades
sed -i 's/losing_trades/losingTrades/g' src/api/server.rs

echo "✅ Nombres de campos corregidos"
echo "🚀 Reconstruyendo proyecto..."
cargo clean
cargo build --release