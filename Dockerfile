FROM rust:bookworm as builder

ARG EXTRA_FEATURES=""

WORKDIR /open_router

ENV CARGO_NET_RETRY=10
ENV RUSTUP_MAX_RETRIES=10
ENV CARGO_INCREMENTAL=0

RUN apt-get update \
    && apt-get install -y libpq-dev libssl-dev pkg-config protobuf-compiler clang

COPY . .
RUN RUSTFLAGS="-A warnings" cargo build --release --features release ${EXTRA_FEATURES}


FROM debian:bookworm

ARG CONFIG_DIR=/local/config
ARG BIN_DIR=/local
ARG BINARY=open_router

RUN apt-get update \
    && apt-get install -y ca-certificates tzdata libpq-dev curl procps libmariadb-dev

EXPOSE 8080

RUN mkdir -p ${CONFIG_DIR}

COPY --from=builder /open_router/target/release/${BINARY} ${BIN_DIR}/${BINARY}

WORKDIR ${BIN_DIR}

CMD ./open_router

