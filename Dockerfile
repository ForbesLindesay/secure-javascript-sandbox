FROM rust:1.99.1-trixie@sha256:931c2c0531a8b143e044f0f3914dfe802f15bb3d1955f0b657e9359327ac2ffc AS builder

WORKDIR /usr/src/secure-javascript-sandbox

# RUN rustup target add wasm32-wasip2

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./crates ./crates

# RUN cargo build --bin secure_js_sandbox_interpreter_boa --release --target wasm32-wasip2
RUN cargo build --release

FROM debian:trixie-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad

# RUN apt-get update && apt-get install -y extra-runtime-dependencies && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/secure-javascript-sandbox/target/release/secure_js_sandbox_server /usr/local/bin/secure_js_sandbox_server
ENTRYPOINT ["secure_js_sandbox_server"]