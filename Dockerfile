FROM rust:latest AS builder
WORKDIR /usr/src/emissary

COPY . .

WORKDIR /usr/src/emissary

RUN apt-get update && \
    apt-get install -y \
        cmake \
        libfontconfig1-dev \
        libglib2.0-dev \
        libgtk-3-dev \
        libwebkit2gtk-4.1-dev \
        libssl-dev \
        libxdo-dev \
        libayatana-appindicator3-dev && \
    rm -rf /var/lib/apt/lists/*

RUN cargo build --release --bin emissary-cli

FROM ubuntu:24.04
RUN apt-get update && apt-get install -y \
    libglib2.0-0 \
    libgtk-3-0 \
    libwebkit2gtk-4.1-0 \
    libxdo3 \
    libssl3 \
    libfontconfig1 \
    libayatana-appindicator3-1 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/emissary/target/release/emissary-cli /usr/local/bin/emissary-cli
RUN chmod +x /usr/local/bin/emissary-cli

ENTRYPOINT ["/usr/local/bin/emissary-cli"]
