FROM rust:1-slim as builder

WORKDIR /usr/src/app

RUN apt-get update && apt-get install -y pkg-config libssl-dev curl && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock Rocket.toml ./
COPY src ./src
COPY migrations ./migrations

RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

RUN apt-get update && apt-get install -y ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 10001 -s /bin/sh appuser

COPY --from=builder /usr/src/app/target/release/spa-sax-backend /app/spa-sax-backend
COPY --from=builder /usr/src/app/target/release/create_admin /app/create_admin
COPY --from=builder /usr/src/app/target/release/migrate /app/migrate
COPY Rocket.toml ./
COPY migrations ./migrations

RUN mkdir -p /app/uploads && chown -R appuser:appuser /app

USER appuser

EXPOSE 8000

HEALTHCHECK --interval=10s --timeout=3s --retries=3 \
  CMD curl -f http://localhost:8000/health || exit 1

CMD ["/app/spa-sax-backend"]
