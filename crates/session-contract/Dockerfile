FROM rust:1-bookworm@sha256:828077e0f5ed0401fbd9cb5b4d5dedca23bd13c7fe032f3e8b7313e7acd2a57f AS development
RUN rustup component add rustfmt clippy
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY fixtures ./fixtures
COPY schemas ./schemas
RUN cargo test --locked --package longcut-session-contract

FROM development AS check
RUN cargo fmt --package longcut-session-contract -- --check \
    && cargo clippy --locked --package longcut-session-contract --all-targets -- -D warnings \
    && cargo test --locked --package longcut-timebase
ENTRYPOINT ["/src/target/debug/session-inspect"]
CMD ["--help"]
