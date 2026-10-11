# Imagen de producción del bot (trading + API + dashboard + Telegram).
#   docker compose build      (ver docker-compose.yml)
#
# Etapas: dashboard (Node) → bot (Rust) → runtime mínimo (Debian slim, sin root).
# El builder de Rust y el runtime deben compartir versión de Debian (glibc/libssl).

# ---------- 1. Dashboard ----------
FROM node:22-slim AS web
WORKDIR /web
COPY web_dashboard/package.json web_dashboard/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web_dashboard/ ./
RUN npm run build

# ---------- 2. Bot ----------
FROM rust:1-trixie AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
# Caché de BuildKit: las recompilaciones solo rehacen lo que cambió
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked \
 && cp target/release/hyperliquid-mmaker-bot /usr/local/bin/arbibot

# ---------- 3. Runtime ----------
FROM debian:trixie-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3t64 curl tini \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /app --create-home arbibot \
 && mkdir -p /app/data /app/web_dashboard \
 && chown -R arbibot /app/data

WORKDIR /app
COPY --from=build /usr/local/bin/arbibot /usr/local/bin/arbibot
COPY --from=web /web/dist /app/web_dashboard/dist

USER arbibot
ENV API_BIND=0.0.0.0 \
    API_PORT=3001 \
    DATA_DIR=/app/data \
    RUST_LOG=info
EXPOSE 3001
VOLUME ["/app/data"]

# El dashboard no requiere token: sirve para comprobar que la API responde
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD curl -fsS -o /dev/null http://127.0.0.1:3001/dashboard/ || exit 1

# SIGINT = Ctrl+C: el bot cancela órdenes pendientes antes de salir
STOPSIGNAL SIGINT
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/arbibot"]
