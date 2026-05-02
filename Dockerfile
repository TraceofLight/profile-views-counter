FROM rust:1-slim-bookworm AS builder
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./

RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src target/release/deps/profile_views_counter*

COPY migrations ./migrations
COPY src ./src

RUN cargo build --release


FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd --system app \
    && useradd --system --gid app --home-dir /home/app --create-home --shell /usr/sbin/nologin app \
    && mkdir -p /data \
    && chown -R app:app /data

COPY --from=builder /app/target/release/profile-views-counter /usr/local/bin/profile-views-counter

USER app
WORKDIR /home/app

ENV DATABASE_URL=sqlite:/data/counter.db?mode=rwc
ENV PORT=3000
ENV RUST_LOG=info

EXPOSE 3000
VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/profile-views-counter"]
