FROM longcut-story-build:latest AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --locked --release --bin longcut-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ffmpeg fonts-dejavu-core ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home longcut \
    && mkdir /data && chown longcut:longcut /data
COPY --from=build /build/target/release/longcut-server /usr/local/bin/longcut-server
USER longcut
ENV LONGCUT_BIND=0.0.0.0:4000 LONGCUT_DATA=/data RUST_LOG=info
CMD ["longcut-server"]
