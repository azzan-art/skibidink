FROM rust:1-slim-bookworm AS server
WORKDIR /src
COPY server-rs/Cargo.toml server-rs/Cargo.lock ./
COPY server-rs/src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim AS css
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN bin/css --minify

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tzdata && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY public ./public
COPY --from=css /app/public/styles/output.css ./public/styles/output.css
COPY --from=server /src/target/release/skibidink /usr/local/bin/skibidink
ENV NODE_ENV=production
ENV TZ=Asia/Manila
CMD ["skibidink"]
