# Running the Decision Engine with PostgreSQL

This guide provides instructions on how to set up and run the Decision Engine application using PostgreSQL as the database.

## Prerequisites

*   Docker and Docker Compose must be installed on your system (OrbStack can be used as an alternative to Docker Desktop).
*   You should have the project code cloned to your local machine.
*   Rust and Cargo should be installed if you intend to build the application with specific features.

## Configuration

To use PostgreSQL, the application needs to be compiled with the `postgres` Rust feature flag enabled.

The PostgreSQL connection details are configured in `config/docker-configuration.toml` under the `[pg_database]` section. The `docker-compose.yaml` file defines a `postgresql` service (`open-router-postgres`) with the following default environment variables:
*   `POSTGRES_USER=db_user`
*   `POSTGRES_PASSWORD=db_pass`
*   `POSTGRES_DB=decision_engine_db`

The service is exposed on port `5432`. The application, when running inside Docker, connects to PostgreSQL using the hostname `postgresql` (as specified in `config/docker-configuration.toml`), which is the service name within the Docker network.

## Setup and Running

Follow these steps to get the application running with PostgreSQL:

### 1. Enable the `postgres` Feature Flag

To use PostgreSQL, the `postgres` feature flag must be enabled. This is typically done by modifying the `Cargo.toml` file.

Open `Cargo.toml` and locate the `[features]` section. Add `postgres` to the `default` features list (if not already present):

```toml
[features]
default = ["middleware", "postgres"] # Ensure "postgres" is here
# ... other features
```

**Important:** After modifying `Cargo.toml`, the application needs to be rebuilt for the changes to take effect. The `make init-local-pg` command (described below) handles the build process. Note that as per the `Cargo.toml` provided, `postgres` is already part of the `default` features, so you might not need to change this unless your `Cargo.toml` differs.

### 2. Start Services and Run Migrations for PostgreSQL

To build the application locally with the `postgres` feature (implicitly, as the build process will pick up the modified `Cargo.toml` if `postgres` is in `default`) and run PostgreSQL migrations:
```bash
make init-local-pg
```

**Explanation:**
*   This command first executes the `db-migrator-postgres` service defined in `docker-compose.yaml`.
*   The `db-migrator-postgres` service connects to the `postgresql` service (container name `open-router-postgres`).
*   It then applies the database schema by executing the SQL script located at `migrations_pg/00000000000000_diesel_postgresql_initial_setup/up.sql`.
*   After the migrations are successfully applied, the command starts the `open-router-local` application service.
*   Because `postgres` is (presumably) in the default features in `Cargo.toml`, the `docker-compose up --build open-router-local` part of the `make` target will rebuild the application with PostgreSQL support.

### 3. Accessing the Application

Once the services are up and running, the Decision Engine API will be available at:
`http://localhost:8080`

## Stopping the Application

To stop all running services and remove the containers:
```bash
make stop
```

## How It Works (Summary)

*   The `postgres` feature flag in `Cargo.toml` controls whether the application is compiled with PostgreSQL support. It is included in the `default` features, meaning it's enabled by default unless you customize the build.
*   The `Makefile` target `init-local-pg` orchestrates the setup process for PostgreSQL.
*   `docker-compose.yaml` defines the `postgresql` service for the database and a `db-migrator-postgres` service to apply schema migrations specific to PostgreSQL.
*   The application's Rust code in `src/storage.rs` conditionally compiles to use PostgreSQL connection logic when the `postgres` feature flag is active.
*   The `config/docker-configuration.toml` file provides the necessary runtime connection parameters for the application to connect to the PostgreSQL database.

## Other Available Feature Flags

Besides `postgres`, the application supports several other feature flags that can be enabled in `Cargo.toml` to include additional functionalities. Here are some notable ones:

*   `mysql`: Enables support for MySQL as an alternative database.
*   `kms-aws`: Integrates with AWS Key Management Service for managing encryption keys.
*   `kms-hashicorp-vault`: Integrates with HashiCorp Vault for secrets management.
*   `external_key_manager`: Enables a generic external key manager interface.
*   `external_key_manager_mtls`: Adds mTLS support for the external key manager.
*   `console`: Enables a Tokio console for debugging and tracing application behavior.
*   `limit`: Potentially related to request limiting or resource constraints (its exact use may require code inspection).
*   `middleware`: Enables common middleware components (included in `default`).
*   `release`: A meta-feature often used to group features for release builds, currently includes `middleware` and `kms-aws`.

To enable a specific feature, you can add it to the `default` list in `Cargo.toml` or specify it during the build process (e.g., `cargo build --features your_feature`). Refer to the `Cargo.toml` file for the complete list and their dependencies.
