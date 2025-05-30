# Running the Decision Engine with PostgreSQL

This guide provides instructions on how to set up and run the Decision Engine application using PostgreSQL as the database.

## Prerequisites

*   Docker and Docker Compose must be installed on your system (OrbStack can be used as an alternative to Docker Desktop).
*   You should have the project code cloned to your local machine.
*   Rust and Cargo should be installed if you intend to build the application with specific features.

## Configuration

To use PostgreSQL, the application needs to be compiled with the `db_migration` Rust feature flag enabled.

The PostgreSQL connection details are configured in `config/docker-configuration.toml` under the `[pg_database]` section. The `docker-compose.yaml` file defines a `postgresql` service (`open-router-postgres`) with the following default environment variables:
*   `POSTGRES_USER=db_user`
*   `POSTGRES_PASSWORD=db_pass`
*   `POSTGRES_DB=decision_engine_db`

The service is exposed on port `5432`. The application, when running inside Docker, connects to PostgreSQL using the hostname `postgresql` (as specified in `config/docker-configuration.toml`), which is the service name within the Docker network.

## Setup and Running

Follow these steps to get the application running with PostgreSQL:

### 1. Enable the `db_migration` Feature Flag

To use PostgreSQL, the `db_migration` feature flag must be enabled. This is typically done by modifying the `Cargo.toml` file.

Open `Cargo.toml` and locate the `[features]` section. Add `db_migration` to the `default` features list:

```toml
[features]
default = ["middleware", "db_migration"] # Add "db_migration" here
# ... other features
```

**Important:** After modifying `Cargo.toml`, the application needs to be rebuilt for the changes to take effect. The `make init-local-pg` command (described below) handles the build process.

### 2. Start Services and Run Migrations for PostgreSQL

To build the application locally with the `db_migration` feature (implicitly, as the build process will pick up the modified `Cargo.toml`) and run PostgreSQL migrations:
```bash
make init-local-pg
```

**Explanation:**
*   This command first executes the `db-migrator-postgres` service defined in `docker-compose.yaml`.
*   The `db-migrator-postgres` service connects to the `postgresql` service (container name `open-router-postgres`).
*   It then applies the database schema by executing the SQL script located at `migrations_pg/00000000000000_diesel_postgresql_initial_setup/up.sql`.
*   After the migrations are successfully applied, the command starts the `open-router-local` application service.
*   Because you've (presumably) modified `Cargo.toml` to include `db_migration` in the default features, the `docker-compose up --build open-router-local` part of the `make` target will rebuild the application with PostgreSQL support.

### 3. Accessing the Application

Once the services are up and running, the Decision Engine API will be available at:
`http://localhost:8080`

## Stopping the Application

To stop all running services and remove the containers:
```bash
make stop
```

## How It Works (Summary)

*   The `db_migration` feature flag in `Cargo.toml` controls whether the application is compiled with PostgreSQL support. You must add this to the `default` features or build with this feature explicitly.
*   The `Makefile` target `init-local-pg` orchestrates the setup process for PostgreSQL.
*   `docker-compose.yaml` defines the `postgresql` service for the database and a `db-migrator-postgres` service to apply schema migrations specific to PostgreSQL.
*   The application's Rust code in `src/storage.rs` conditionally compiles to use PostgreSQL connection logic when the `db_migration` feature flag is active.
*   The `config/docker-configuration.toml` file provides the necessary runtime connection parameters for the application to connect to the PostgreSQL database.
