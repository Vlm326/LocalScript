FROM rust:1.85 as builder
WORKDIR /app 
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/sandbox-service .
EXPOSE 6778
CMD ["./sandbox-service"]
