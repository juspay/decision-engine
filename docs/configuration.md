# Configuration Guide

Dynamo uses TOML configuration files to customize its behavior. This guide explains the available configuration options and how to use them effectively.

## Configuration File Location

By default, Dynamo looks for configuration files in the following locations:

- Development mode: `config/development.toml`
- Production mode: `config/production.toml`

You can also specify a custom configuration file using environment variables.

## Configuration Format

Below is an explanation of the main configuration sections:

### Server Configuration

```toml
[server]
host = "127.0.0.1"      # The address to bind the server to
port = 8000             # The port to listen on
type = "grpc"           # Server type: "grpc" or "http"
```

### Metrics Server Configuration

```toml
[metrics]
host = "127.0.0.1"      # The address to bind the metrics server to
port = 9000             # The port for the metrics server
```

### Logging Configuration

```toml
[log.console]
enabled = true          # Whether to enable console logging
level = "DEBUG"         # Log level: DEBUG, INFO, WARN, ERROR
log_format = "default"  # Log format: "default" or "json"
```

### Redis Configuration

```toml
[redis]
host = "127.0.0.1"
port = 6379
pool_size = 5                      # Number of connections to keep open
reconnect_max_attempts = 5         # Maximum reconnection attempts
reconnect_delay = 5                # Delay between attempts in milliseconds
use_legacy_version = false         # Use RESP2 protocol for Redis < 6
```

### TTL for Keys

```toml
[ttl_for_keys]
aggregates = 300                   # Time to live for aggregates keys (seconds)
current_block = 900                # Time to live for current_block keys (seconds)
elimination_bucket = 900           # Time to live for elimination buckets (seconds)
contract_ttl = 900                 # Time to live for contracts (seconds)
```

### Global Routing Configurations

```toml
[global_routing_configs.success_rate]
min_aggregates_size = 5            # Minimum number of buckets for SR calculation
default_success_rate = 100         # Default SR when insufficient data
max_aggregates_size = 10           # Maximum number of aggregates to store

[global_routing_configs.success_rate.current_block_threshold]
duration_in_mins = 10              # Current block duration in minutes
max_total_count = 5                # Maximum transaction count for current block

[global_routing_configs.elimination_rate]
bucket_size = 5                    # Capacity of buckets
bucket_leak_interval_in_secs = 300 # Leak rate of buckets in seconds
```

### Multi-Tenancy

```toml
[multi_tenancy]
enabled = true                     # Enable multi-tenant mode
```

## Environment Variables

You can override configuration values using environment variables with the prefix `DYNAMO__`:

```bash
# Override Redis host and port
export DYNAMO__REDIS__HOST=redis-server
export DYNAMO__REDIS__PORT=6380

# Override server port
export DYNAMO__SERVER__PORT=9000
```

## Configuration Examples

### Development Configuration

```toml
[log.console]
enabled = true
level = "DEBUG"
log_format = "default"

[server]
host = "127.0.0.1"
port = 8000
type = "grpc"

[redis]
host = "127.0.0.1"
port = 6379
```

### Production Configuration

```toml
[log.console]
enabled = true
level = "INFO"
log_format = "json"

[server]
host = "0.0.0.0"  # Listen on all interfaces
port = 8000
type = "grpc"

[redis]
host = "redis-server"  # Use service name in production
port = 6379
pool_size = 20
reconnect_max_attempts = 10
```

### High-Availability Configuration

```toml
[server]
host = "0.0.0.0"
port = 8000
type = "grpc"

[redis]
host = "redis-master"
port = 6379
pool_size = 30
reconnect_max_attempts = 15
reconnect_delay = 2
```

## Advanced Configuration

### Using Multiple Redis Instances

While not directly supported in the configuration file, you can set up Redis Sentinel or Redis Cluster for high availability and implement a custom client.

### Configuring Routing Strategies

Each routing strategy has specific configuration parameters:

1. **Success Rate Routing**: Configure weight factors, time windows, and default values
2. **Elimination Routing**: Configure bucket sizes, leak rates, and thresholds
3. **Contract Routing**: Configure contract scores, targets, and time scales

## Next Steps

- [Installation Guide](setup-guide.md)
- [API Reference](api-reference.md)
