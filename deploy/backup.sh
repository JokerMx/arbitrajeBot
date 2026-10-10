#!/bin/bash
# Respalda usuarios, pagos e historial de señales (DATA_DIR) y conserva 30 días.
# Cron diario sugerido (crontab -e):
#   0 4 * * * /opt/arbitrajeBot/deploy/backup.sh >> /opt/arbitrajeBot/logs/backup.log 2>&1
set -euo pipefail

cd "$(dirname "$0")/.."
DATA_DIR=$(grep -E '^DATA_DIR=' .env 2>/dev/null | cut -d= -f2- | tr -d '\r' || true)
DATA_DIR=${DATA_DIR:-./data}
DEST=${BACKUP_DIR:-./backups}

mkdir -p "$DEST"
STAMP=$(date +%Y%m%d_%H%M%S)
tar -czf "$DEST/data_$STAMP.tar.gz" -C "$DATA_DIR" . 2>/dev/null || {
    echo "⚠️  No se encontró $DATA_DIR (¿primer arranque?)"
    exit 0
}
find "$DEST" -name 'data_*.tar.gz' -mtime +30 -delete
echo "✅ Respaldo: $DEST/data_$STAMP.tar.gz"
