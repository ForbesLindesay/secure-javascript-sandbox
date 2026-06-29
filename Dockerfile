FROM rust:1.96.0-trixie@sha256:c6811167278337db5f3b0234964ced5f538f154a2a20f09ec03721d7411c933d AS builder

WORKDIR /usr/src/secure-javascript-sandbox

# RUN rustup target add wasm32-wasip2

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./crates ./crates

# RUN cargo build --bin secure_js_sandbox_interpreter_boa --release --target wasm32-wasip2
RUN cargo build --release

FROM debian:trixie-slim@sha256:4e401d95de7083948053197a9c3913343cd06b706bf15eb6a0c3ccd26f436a0e

# RUN apt-get update && apt-get install -y extra-runtime-dependencies && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/secure-javascript-sandbox/target/release/secure_js_sandbox_server /usr/local/bin/secure_js_sandbox_server
ENTRYPOINT ["secure_js_sandbox_server"]