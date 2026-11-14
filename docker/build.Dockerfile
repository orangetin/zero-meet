# syntax=docker/dockerfile:1
FROM rust:1.99.1-bookworm@sha256:e6f2cff35d0f309acf8433b4f05878ed77092602c5065cc81801177da0bb3a9c AS build

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

FROM gcr.io/distroless/cc-debian13:latest@sha256:809d04afa1be033c441f76bb0051878c342fb76d0ea236bc6dc94b655fa0fbd6

COPY --from=build /zero-meet-server /zero-meet-server
COPY --from=build --chown=65532:65532 /data /data

USER 65532:65532
ENTRYPOINT ["/zero-meet-server"]
CMD ["/data/server.key"]
