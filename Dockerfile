FROM rust:latest AS builder
WORKDIR /usr/src/emissary

COPY . .

WORKDIR /usr/src/emissary

RUN apt-get update && apt-get install -y cmake && rm -rf /var/lib/apt/lists/*

RUN cargo build --release --no-default-features --features web-ui --bin emissary-cli

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/emissary/target/release/emissary-cli /usr/local/bin/emissary-cli
RUN chmod +x /usr/local/bin/emissary-cli

ENTRYPOINT ["/usr/local/bin/emissary-cli"]
