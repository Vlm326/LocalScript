FROM rust:1.85 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/llm-tui .
ENV LLM_SERVICE_URL=http://llm-service:8080
ENTRYPOINT ["./llm-tui"]
