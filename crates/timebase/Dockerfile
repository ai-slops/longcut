FROM rust:1.85-bookworm AS test
RUN rustup component add rustfmt clippy
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo test --locked --package longcut-timebase
RUN cargo fmt --package longcut-timebase -- --check \
    && cargo clippy --locked --package longcut-timebase --all-targets --all-features -- -D warnings
CMD ["cargo", "run", "--locked", "--package", "longcut-timebase", "--example", "time_map"]
