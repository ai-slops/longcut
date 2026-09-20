FROM rust:1-bookworm@sha256:828077e0f5ed0401fbd9cb5b4d5dedca23bd13c7fe032f3e8b7313e7acd2a57f AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --locked --release --bin longcut-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ffmpeg fonts-dejavu-core ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home longcut \
    && mkdir /data && chown longcut:longcut /data
COPY --from=build /build/target/release/longcut-server /usr/local/bin/longcut-server
USER longcut
ENV LONGCUT_BIND=0.0.0.0:4000 LONGCUT_DATA=/data RUST_LOG=info
EXPOSE 4000
VOLUME /data
CMD ["longcut-server"]

FROM build AS check
RUN rustup component add rustfmt clippy
RUN apt-get update && apt-get install -y --no-install-recommends ffmpeg fonts-dejavu-core \
    && rm -rf /var/lib/apt/lists/*
RUN cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
