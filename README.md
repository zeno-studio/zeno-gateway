# Zeno Gateway

A high-performance API gateway built with Rust.

## Development

```bash
# Build
cargo build

# Run tests
cargo test

# Run locally
cargo run
```

## Docker Build

To build the Docker image locally:

```bash
docker build -t zeno-gateway .
```

## Deployment

### Prerequisites

1. Podman installed on the server
2. Access to GitHub Container Registry (GHCR)
3. Proper permissions to pull the container image

### Setup

1. Copy the `deploy.sh` script to your server
2. Make it executable:
   ```bash
   chmod +x deploy.sh
   ```
3. Run the deployment script:
   ```bash
   ./deploy.sh
   ```
4. The script will create necessary directories and a default environment file if they don't exist
5. Edit the environment file at `/etc/zeno-gateway/.env` with your actual configuration
6. Run the deployment script again to start the container

### Updating

To update to the latest version:

```bash
./deploy.sh
```

### Viewing Logs

```bash
podman logs -f zeno-gateway
```

## Environment Variables

The following environment variables can be configured in `/etc/zeno-gateway/.env`:

- `RUST_LOG`: Logging level (default: info)
- `ENABLE_HTTPS`: Enable HTTPS (true/false)
- `DOMAIN`: Your domain name
- `ACME_CONTACT`: Email for Let's Encrypt
- `ACME_DIRECTORY`: ACME directory URL
- `ANKR_API_KEY`: Ankr API key
- `BLAST_API_KEY`: Blast API key
- `OPENEXCHANGE_KEY`: OpenExchange API key