# syntax=docker/dockerfile:1
FROM rust:1.99.0-bookworm@sha256:114c7a4425406451c2866b6aafe69fe29b1b298832db1277d411ac73c82d04d6 AS build

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY meet ./meet

COPY app/Cargo.toml ./app/Cargo.toml
COPY app/src/main.rs ./app/src/main.rs

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target,sharing=locked \
    cargo build --locked --release -p zero-meet --bin zero-meet-server && \
    cp target/release/zero-meet-server /zero-meet-server

RUN mkdir /data

FROM gcr.io/distroless/cc-debian13:latest@sha256:d74d144baa124dd9defd01d71872bb56c49d4441dba6153ccf790364f97e64b1

COPY --from=build /zero-meet-server /zero-meet-server
COPY --from=build --chown=65532:65532 /data /data

USER 65532:65532
ENTRYPOINT ["/zero-meet-server"]
CMD ["/data/server.key"]
