FROM rust:1-bookworm@sha256:828077e0f5ed0401fbd9cb5b4d5dedca23bd13c7fe032f3e8b7313e7acd2a57f AS check
RUN rustup component add rustfmt clippy \
    && apt-get update && apt-get install -y --no-install-recommends ffmpeg \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY fixtures ./fixtures
COPY schemas ./schemas
RUN cargo fmt -p longcut-session-media -p longcut-session-contract -- --check \
    && cargo clippy --locked -p longcut-session-media -p longcut-session-contract --all-targets -- -D warnings \
    && cargo test --locked -p longcut-session-media -p longcut-session-contract
RUN cargo run --locked -p longcut-session-media --example make_fixture -- /tmp/session-media-demo \
    && cargo run --locked -p longcut-session-media --example make_out_of_bounds_fixture -- /tmp/session-media-overrun \
    && chown -R 65534:65534 /tmp/session-media-demo /tmp/session-media-overrun /src/fixtures
USER 65534:65534
ENTRYPOINT ["/src/target/debug/session-media-inspect"]
CMD ["--help"]
