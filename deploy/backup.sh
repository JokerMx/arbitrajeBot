#!/bin/bash
# Respaldo diario: base de datos (señales, operaciones, usuarios, pagos, posiciones,
# configuración y balance) y la carpeta de datos. Conserva 30 días.
# Cron sugerido (crontab -e):
#   0 4 * * * /opt/arbitrajeBot/deploy/backup.sh >> /opt/arbitrajeBot/logs/backup.log 2>&1
set -euo pipefail

cd "$(dirname "$0")/.."
DEST=${BACKUP_DIR:-./backups}
STAMP=$(date +%Y%m%d_%H%M%S)
mkdir -p "$DEST"

# 1. Base de datos
if docker compose ps --status running --services 2>/dev/null | grep -qx db; then
    # MariaDB incluida en docker-compose.yml
    docker compose exec -T db sh -c 'mariadb-dump --single-transaction -uarbibot -p"$MARIADB_PASSWORD" arbibot' \
        | gzip > "$DEST/bd_$STAMP.sql.gz"
    echo "✅ Base de datos: $DEST/bd_$STAMP.sql.gz"
else
    # Base de datos propia: usa DATABASE_URL del .env (mysql://usuario:clave@host:puerto/base)
    URL=$(grep -E '^DATABASE_URL=' .env | cut -d= -f2- | tr -d '\r')
    if [[ "$URL" =~ ^mysql://([^:]+):([^@]*)@([^:/]+):?([0-9]*)/(.+)$ ]]; then
        MYSQL_PWD="${BASH_REMATCH[2]}" mysqldump --single-transaction -u "${BASH_REMATCH[1]}" \
            -h "${BASH_REMATCH[3]}" -P "${BASH_REMATCH[4]:-3306}" "${BASH_REMATCH[5]}" \
            | gzip > "$DEST/bd_$STAMP.sql.gz"
        echo "✅ Base de datos: $DEST/bd_$STAMP.sql.gz"
    else
        echo "⚠️  No se pudo interpretar DATABASE_URL; se omite el respaldo de la BD"
    fi
fi

# 2. Carpeta de datos (archivos de versiones anteriores, ya migrados a la BD)
DATA_DIR=$(grep -E '^DATA_DIR=' .env 2>/dev/null | cut -d= -f2- | tr -d '\r' || true)
DATA_DIR=${DATA_DIR:-./data}
if [ -d "$DATA_DIR" ]; then
    tar -czf "$DEST/data_$STAMP.tar.gz" -C "$DATA_DIR" . 2>/dev/null || true
fi

find "$DEST" \( -name 'bd_*.sql.gz' -o -name 'data_*.tar.gz' -o -name 'subscribers_root_*.json' \) -mtime +30 -delete
echo "✅ Respaldo terminado"
