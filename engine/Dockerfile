# Multi-arch container — v0.8 8-V6.
#
# Build with `docker buildx build --platform linux/amd64,linux/arm64 -t argos/argos:0.8.0 .`
#
# Build stage
FROM --platform=$BUILDPLATFORM rust:1.94-slim AS builder
WORKDIR /build
COPY . .
RUN cargo build --release -p argos-server && strip target/release/argos

# Runtime stage — distroless-like minimal
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/argos /usr/local/bin/argos
RUN mkdir -p /data
EXPOSE 8080 9200 8081
ENV ARGOS_LOG=info

# argos binds loopback by default (issue #228) so that a bare `argos` cannot
# publish an API key in cleartext to every interface. A container has to bind
# 0.0.0.0 — the container's network namespace is itself the boundary, and
# nothing reaches the listeners until you publish a port or route a Service to
# it. That published port is where TLS belongs: terminate it at your ingress,
# proxy or mesh. The declaration is explicit here rather than defaulted into
# the engine, and the startup banner names the exposure on every boot.
#
# Override either variable to change it (`docker run -e ARGOS_BIND_ADDRESS=...`).
ENV ARGOS_BIND_ADDRESS=0.0.0.0
ENV ARGOS_ALLOW_INSECURE_NETWORK_BIND=true

# Container HEALTHCHECK uses the v0.8 readiness probe — kubelet hits the
# same endpoint via Helm-chart probe configs.
HEALTHCHECK --interval=10s --timeout=2s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1:9200/health/ready || exit 1

ENTRYPOINT ["argos"]
CMD ["--insecure", "--data-dir", "/data"]
