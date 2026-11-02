FROM rust:bookworm as builder

ARG EXTRA_FEATURES=""

WORKDIR /open_router

ENV CARGO_NET_RETRY=10
ENV RUSTUP_MAX_RETRIES=10
ENV CARGO_INCREMENTAL=0

RUN apt-get update \
    && apt-get install -y cmake libpq-dev libssl-dev pkg-config protobuf-compiler clang

COPY . .
RUN RUSTFLAGS="-A warnings" cargo build --release --features release ${EXTRA_FEATURES}


FROM debian:bookworm

ARG CONFIG_DIR=/local/config
ARG BIN_DIR=/local
ARG BINARY=open_router
ENV BINARY=${BINARY}

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata libpq-dev curl procps libmariadb-dev

EXPOSE 8080

# Unprivileged runtime user. The UID is numeric so Kubernetes can verify
# `runAsNonRoot` without resolving names inside the image.
ARG APP_UID=10001
RUN groupadd --system --gid ${APP_UID} app \
    && useradd --system --uid ${APP_UID} --gid app --no-create-home --shell /usr/sbin/nologin app \
    && mkdir -p ${CONFIG_DIR}

COPY --from=builder /open_router/target/release/${BINARY} ${BIN_DIR}/${BINARY}

USER ${APP_UID}:${APP_UID}

WORKDIR ${BIN_DIR}

CMD ["sh", "-c", "./${BINARY}"]
