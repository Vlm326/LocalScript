FROM rust:1.85 as builder
WORKDIR /app 
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/sandbox-service .
EXPOSE 6778
CMD ["./sandbox-service"]
