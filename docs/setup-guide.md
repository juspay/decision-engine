# Installation Guide

This guide provides detailed instructions for installing Dynamo on different platforms.

## Prerequisites

- Rust 1.78.0 or later
- Redis server 6.0 or later
- Git

## From Source

### 1. Clone the Repository

```bash
git clone https://github.com/yourusername/dynamo.git
cd dynamo
```

### 2. Install Dependencies

#### Ubuntu/Debian

```bash
# Install system dependencies
sudo apt-get update
sudo apt-get install -y pkg-config libssl-dev protobuf-compiler libpq-dev

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

#### macOS

```bash
# Using Homebrew
brew install pkg-config openssl protobuf postgresql

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 3. Build Dynamo

```bash
# Build in release mode
cargo build --release
```

### 4. Set Up Configuration

```bash
# Copy example configuration
cp config/config.example.toml config/development.toml

# Edit configuration as needed
# vi config/development.toml
```

### 5. Run Redis

Make sure Redis is running on your system:

```bash
# Install Redis if needed
sudo apt-get install redis-server  # Ubuntu/Debian
brew install redis                 # macOS

# Start Redis server
redis-server
```

### 6. Run Dynamo

```bash
# Run with development configuration
./target/release/dynamo
```

## Using Docker

### 1. Pull the Docker Image

```bash
docker pull yourusername/dynamo:latest
```

Or build it yourself:

```bash
docker build -t dynamo:latest .
```

### 2. Run with Docker

```bash
# Run with default configuration
docker run -p 8000:8000 -p 9000:9000 dynamo:latest

# Run with custom configuration
docker run -p 8000:8000 -p 9000:9000 \
  -v $(pwd)/config:/app/config \
  dynamo:latest
```

### 3. Docker Compose Setup

Create a `docker-compose.yml` file:

```yaml
version: '3'
services:
  dynamo:
    image: yourusername/dynamo:latest
    ports:
      - "8000:8000"
      - "9000:9000"
    volumes:
      - ./config:/app/config
    depends_on:
      - redis
  
  redis:
    image: redis:latest
    ports:
      - "6379:6379"
```

Run with Docker Compose:

```bash
docker-compose up -d
```

## Building the WebAssembly Module

The `procesmo` module can be built for web use:

```bash
# Install wasm-pack if needed
cargo install wasm-pack

# Build the WebAssembly module
cd crates/procesmo
wasm-pack build --target web
```

## Verifying Installation

To verify that Dynamo is running correctly:

```bash
# Check health status
curl http://localhost:8000/grpc.health.v1.Health/Check

# Or using grpcurl
grpcurl -plaintext localhost:8000 grpc.health.v1.Health/Check
```

You should see a response indicating the service is running.

## Next Steps

- [Configure Dynamo](configuration.md) for your environment
- [Explore the API](api-reference.md)
