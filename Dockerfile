FROM rust:slim-bookworm as base
RUN cargo install cargo-chef --version ^0.1
RUN apt-get update \
    && apt-get install -y pkg-config libssl-dev
RUN cargo install sccache

FROM base AS planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM base AS builder
WORKDIR /app
COPY --from=planner /app/recipe.json recipe.json

# Set up sccache
# ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
ENV SCCACHE_DIR=/sccache
ENV SCCACHE_CACHE_SIZE=5G

# Cook dependencies
RUN --mount=type=cache,target=/sccache \
    cargo chef cook --release --recipe-path recipe.json

RUN apt-get update \
    && apt-get install -y protobuf-compiler libpq-dev


# Build the application
COPY . .
RUN --mount=type=cache,target=/sccache \
    cargo build --release --features release

# Print sccache stats
RUN sccache --show-stats

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y libpq-dev ca-certificates
RUN mkdir -p bin config

COPY --from=builder /app/target/release/dynamo bin/dynamo
COPY --from=builder /app/target/release/simulator bin/simulator
# allows us to mount `/app/config/production.toml`
COPY --from=builder /app/config config

# Copy the shell entrypoint script and make it executable
COPY entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

# Set the entrypoint script
ENTRYPOINT ["/app/entrypoint.sh"]
