# Decision Engine - Technical Context

## Technology Stack

The Decision Engine is built on a modern, high-performance technology stack:

### Backend

- **Primary Language**: Rust (for high performance, memory safety, and concurrency)
- **Web Framework**: Axum (asynchronous web framework for Rust)
- **Database**: PostgreSQL (for persistent storage)
- **Caching**: Redis (for high-speed data access)
- **Asynchronous Runtime**: Tokio (async runtime for Rust)

### Infrastructure

- **Containerization**: Docker
- **Database Migration**: Diesel (ORM and migration tool for Rust)
- **TLS Support**: RustTLS
- **Memory Allocation**: jemalloc (for improved memory management)
- **Configuration**: TOML-based configuration files

### Development Tools

- **Build System**: Cargo (Rust package manager)
- **Testing Framework**: Rust test framework
- **CI/CD**: Not explicitly defined in the codebase, but likely uses GitHub Actions
- **Version Control**: Git

## Development Environment Setup

### Prerequisites

- Rust toolchain
- Docker and Docker Compose
- PostgreSQL client (for development and debugging)
- Redis client (for development and debugging)

### Setup Process

1. **Clone Repository**:
   ```bash
   git clone {repo-url}
   cd {repo-directory}
   ```

2. **Environment Setup**:
   - Using Docker for local development:
     ```bash
     make init
     ```
   - This sets up the database, Redis, and runs the necessary migrations

3. **Running the Application**:
   - With Docker:
     ```bash
     make run
     ```
   - Without Docker (local code changes):
     ```bash
     make init-local
     make run-local
     ```

4. **Updating Configurations**:
   ```bash
   make update-config
   ```

## Key Technical Components

### Configuration System

The configuration system uses multiple layers:

1. **Base Configuration**: Defined in TOML files (`config/*.toml`)
2. **Environment Overrides**: Environment variables can override config values
3. **Tenant-Specific Configuration**: Allows multi-tenant isolation

Key configuration files:
- `config/development.toml`: Development environment settings
- `config/docker-configuration.toml`: Docker environment settings
- `config.example.toml`: Template for configuration

### Database Schema

The database schema is managed using Diesel migrations:

- `migrations/00000000000000_diesel_initial_setup`: Initial database setup
- `migrations/2025-04-23-103603_add_routing_algorithm_mapper_table`: Routing algorithm mapper
- `migrations/2025-05-09-112540_add_metadata_to_routing_algorithm`: Added metadata to routing algorithm

### Core Modules

1. **API Layer** (`src/routes/`):
   - `decide_gateway.rs`: Gateway decision endpoint
   - `update_gateway_score.rs`: Score update endpoint
   - `health.rs`: Health check endpoints
   - Other endpoints for configuration management

2. **Decider Engine** (`src/decider/`):
   - `gatewaydecider/`: Core decision-making logic
   - `configs/`: Configuration processing
   - `network_decider/`: Network-specific decision logic
   - `storage/`: Decider-specific storage interfaces

3. **Euclid Engine** (`src/euclid/`):
   - `ast.rs`: Abstract Syntax Tree for the rule language
   - `interpreter.rs`: Rule interpreter
   - `handlers/`: API handlers for rule management
   - `cgraph.rs`: Computation graph for rule evaluation

4. **Feedback System** (`src/feedback/`):
   - `gateway_scoring_service.rs`: Service for updating gateway scores
   - `gateway_elimination_scoring.rs`: Logic for eliminating underperforming gateways
   - `gateway_selection_scoring_v3.rs`: Advanced scoring for gateway selection

5. **Storage Layer** (`src/storage/`):
   - `db.rs`: Database connection management
   - `caching.rs`: Caching mechanisms
   - `schema.rs`: Database schema definitions

6. **Redis Integration** (`src/redis/`):
   - `cache.rs`: Redis caching implementation
   - `commands.rs`: Redis command wrappers
   - `feature.rs`: Feature flag management via Redis

7. **Types System** (`src/types/`):
   - Extensive type definitions for various domain objects
   - Currency, card, payment method, and other payment-specific types
   - Merchant and tenant configuration types

8. **Error Handling** (`src/error/`):
   - `custom_error.rs`: Domain-specific error types
   - `container.rs`: Error container for structured error handling
   - `transforms.rs`: Error transformation utilities

### Multi-Tenancy Implementation

The multi-tenancy model is implemented through:

1. **Global App State**: Shared resources across all tenants
   ```rust
   pub struct GlobalAppState {
       pub global_config: GlobalConfig,
       pub tenant_app_states: RwLock<HashMap<String, Arc<TenantAppState>>>,
       pub ready: AtomicBool,
   }
   ```

2. **Tenant App State**: Tenant-specific resources
   ```rust
   pub struct TenantAppState {
       pub db: Storage,
       pub redis_conn: Arc<RedisConnectionWrapper>,
       pub config: config::TenantConfig,
       pub api_client: ApiClient,
   }
   ```

## Performance Considerations

1. **Asynchronous Processing**:
   - All I/O operations are asynchronous for maximum throughput
   - Request handling is fully non-blocking

2. **Connection Pooling**:
   - Database connections are pooled for efficiency
   - Redis connections use connection pooling

3. **Optimized Memory Usage**:
   - Custom allocator (jemalloc) for better memory management
   - Efficient data structures to minimize allocations

4. **Caching Strategy**:
   - Multi-level caching for frequently accessed data
   - Cache invalidation on data updates

## Security Considerations

1. **TLS Support**:
   - Optional TLS encryption for API endpoints
   - Configuration via `tls_config` in application settings

2. **Crypto Module** (`src/crypto/`):
   - Encryption management
   - Hashing utilities
   - Secret management

3. **Tenant Isolation**:
   - Strict separation of tenant data
   - Schema-based isolation in the database

## Monitoring and Logging

The logging system is built on top of the tracing crate:

1. **Structured Logging**:
   - JSON-formatted logs for machine parsing
   - Log levels for filtering

2. **Request Tracing**:
   - Request/response logging with latency tracking
   - Error details for failed requests

3. **Storage Interface**:
   - Optional log storage for persistence
   - Environment-based configuration

## Build and Deployment

The project includes:

1. **Dockerfile**: For containerized deployment
2. **docker-compose.yaml**: For local development and testing
3. **Makefile**: With commands for common operations

Build process:
```bash
# Build the application
cargo build --release

# Run unit tests
cargo test

# Build Docker image
docker build -t decision-engine .
```

## Dependencies

Major dependencies include:

- `axum`: Web framework
- `tokio`: Async runtime
- `diesel`: ORM and migrations
- `redis`: Redis client
- `serde`: Serialization/deserialization
- `tracing`: Logging and instrumentation
- `error-stack`: Error handling

## Testing Approach

1. **Unit Testing**:
   - Standard Rust testing for individual components
   - Mocked dependencies for isolation

2. **Integration Testing**:
   - End-to-end testing of API endpoints
   - Database integration tests

3. **Manual Testing Scripts**:
   - `test_euclid.sh`: Testing script for the Euclid engine
