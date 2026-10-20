# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY migrations ./migrations
RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release --no-default-features --features db-postgres \
    && rm -rf src

COPY src ./src
RUN touch src/main.rs \
    && cargo build --release --no-default-features --features db-postgres

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates procps util-linux \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/jarvis-api /usr/local/bin/jarvis-api

ENV PORT=8080
EXPOSE 8080

USER nobody
CMD ["jarvis-api"]
