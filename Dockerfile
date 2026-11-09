# Volvix Home: build the Rust server, then ship it with the web app in a small image.
FROM rust:1-bookworm AS build
WORKDIR /src/server
COPY server/Cargo.toml server/Cargo.lock ./
# build the dependencies on their own first: this slow layer is cached, so later builds only compile our own code
RUN mkdir src && echo 'fn main() {}' > src/main.rs && cargo build --release && rm -rf src target/release/deps/volvix_home* target/release/volvix-home*
COPY server/src ./src
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates ffmpeg && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/server/target/release/volvix-home /usr/local/bin/volvix-home
COPY web /app/web
ENV VH_MUSIC_DIR=/music \
    VH_DATA_DIR=/data \
    VH_WEB_DIR=/app/web \
    VH_PORT=4747 \
    VH_RESCAN_MINUTES=15
EXPOSE 4747
VOLUME ["/data"]
CMD ["volvix-home"]
