# Build stage
FROM rust:1.70 as builder

# Install required system dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/zeno-gateway

# Copy only the files needed for building
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y openssl ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /usr/src/zeno-gateway/target/release/zeno-gateway /app/

# Create volume for ACME cache
RUN mkdir -p /app/acme-cache && \
    chown nobody:nogroup /app/acme-cache && \
    chmod 755 /app/acme-cache

# Use an unprivileged user
USER nobody

# Expose both HTTP and HTTPS ports
EXPOSE 3000 8443

# Set volumes
VOLUME ["/app/acme-cache"]

# Set the entry point
ENTRYPOINT ["/app/zeno-gateway"]
