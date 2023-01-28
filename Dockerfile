FROM rust:1.66.1-buster as builder
ENV RUST_BACKTRACE 1

WORKDIR /usr/src/rust-mqtt-to-kafka
COPY . .
RUN apt-get update && apt-get install -y git cmake libssl-dev
RUN cargo build --release

FROM debian:buster-slim

RUN apt-get update && apt-get install -y  openssl
COPY --from=builder /usr/src/rust-mqtt-to-kafka/target/release/rust-mqtt-to-kafka /usr/local/bin/rust-mqtt-to-kafka
CMD ["rust-mqtt-to-kafka"]
