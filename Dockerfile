FROM rust:1.66.1-buster as builder
ENV RUST_BACKTRACE 1
ENV TARGET_CC x86_64-linux-musl-gcc
WORKDIR /usr/src/rust-mqtt-to-kafka
COPY . .
RUN apt-get update && apt-get install -y git cmake libssl-dev  musl-dev gcc-x86-64-linux-gnu
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --release --target x86_64-unknown-linux-musl


FROM debian:buster-slim

RUN apt-get update && apt-get install -y  openssl
COPY --from=builder /usr/src/rust-mqtt-to-kafka/target/release/rust-mqtt-to-kafka /usr/local/bin/rust-mqtt-to-kafka
CMD ["rust-mqtt-to-kafka"]
