FROM rust:1.91.1-trixie AS builder

WORKDIR /usr/src/secure-javascript-sandbox

# RUN rustup target add wasm32-wasip2

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./crates ./crates
COPY ./wit ./wit
# RUN cargo build --bin secure_js_sandbox_interpreter_boa --release --target wasm32-wasip2
RUN cargo build --release
RUN cargo install --path crates/server

FROM debian:trixie-slim

# RUN apt-get update && apt-get install -y extra-runtime-dependencies && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/secure_js_sandbox_server /usr/local/bin/secure_js_sandbox_server
ENTRYPOINT ["secure_js_sandbox_server"]