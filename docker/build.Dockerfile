# syntax=docker/dockerfile:1
FROM rust:1.97.1-bookworm@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97 AS build

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

FROM gcr.io/distroless/cc-debian13:latest@sha256:4594d59540d1948417f6ca2829ddd9294493a7c68b7528f4dd459de7f203a750

COPY --from=build /zero-meet-server /zero-meet-server
COPY --from=build --chown=65532:65532 /data /data

USER 65532:65532
ENTRYPOINT ["/zero-meet-server"]
CMD ["/data/server.key"]
