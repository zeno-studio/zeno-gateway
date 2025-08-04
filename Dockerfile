# Build stage
FROM rust:latest as builder

# Install required system dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy only the files needed for building
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build the application
RUN cargo build --release --locked --target x86_64-unknown-linux-musl

# Runtime stage
FROM alpine:latest

# Install necessary runtime dependencies
RUN apk add --no-cache openssl

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/zeno-gateway /app/


# Use an unprivileged user
USER nobody

# Expose both HTTP and HTTPS ports
EXPOSE 3000 443


# Set the entry point
ENTRYPOINT ["/app/zeno-gateway"]
