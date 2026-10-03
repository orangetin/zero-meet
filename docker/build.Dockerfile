# syntax=docker/dockerfile:1
FROM rust:1.98.1-bookworm@sha256:93ce27a88655056a51dbdd8f5f2d7ddc071c7b0070fb288a37b5a285fc83971e AS build

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

FROM gcr.io/distroless/cc-debian13:latest@sha256:159783207c2cd44c2aa5715961d13c8612368ac9bd450f887e3f08fc8ea461e3

COPY --from=build /zero-meet-server /zero-meet-server
COPY --from=build --chown=65532:65532 /data /data

USER 65532:65532
ENTRYPOINT ["/zero-meet-server"]
CMD ["/data/server.key"]
