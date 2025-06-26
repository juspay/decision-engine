# PostgreSQL Setup Guide for Decision Engine

This guide provides instructions on how to set up the Decision Engine with PostgreSQL as the database. There are several ways to achieve this, depending on your preference for using Docker or a local PostgreSQL installation.

## Prerequisites

Before you begin, ensure you have the necessary tools installed based on your chosen setup method.

**Common Tools (potentially needed for all methods):**

*   A text editor or IDE for viewing/editing configuration files.
*   Git for cloning the project repository.
*   Rust and Cargo: Essential to build and run the Decision Engine application from source directly on your host machine.

**For All Docker-Based Setups:**

*   **Docker and Docker Compose**: Essential for running the application and database in containers.


**For Full Local Development Setup:**

*   **PostgreSQL**: Needed if you are managing a local PostgreSQL instance directly (e.g., for creating databases, running `just resurrect`).
*   **Just**: A command runner used for some of the local setup scripts (e.g., `just resurrect`, `just migrate-pg`). You can install it by following the instructions [here](https://github.com/casey/just#installation).
*   **Diesel CLI (with PostgreSQL feature)**: Required for managing database migrations when running against a local PostgreSQL database. Install using:

    ```bash
    cargo install diesel_cli --no-default-features --features postgres
    ```

## Setup Options

Choose one of the following methods to set up the Decision Engine with PostgreSQL:

### 1. Using Pre-built Docker Image (Recommended for quick setup)

This method uses pre-built Docker images for both the application and the PostgreSQL database. It's the simplest way to get started.

**Steps:**

1.  Navigate to the root directory of the `decision-engine` project.
2.  Run the following command:

    ```bash
    make init-pg
    ```

    This command will:
    *   Pull the necessary Docker images.
    *   Start a PostgreSQL container.
    *   Run database migrations using a `db-migrator-postgres` service defined in `docker-compose.yaml`.
    *   Start the Decision Engine application container (`open-router-pg`), configured to connect to the PostgreSQL database.

The application should now be running and accessible.

### 2. Using Local Changes with PostgreSQL in Docker

This method is useful if you are making changes to the Decision Engine codebase and want to test them with a PostgreSQL database running in Docker.

**Steps:**

1.  Navigate to the root directory of the `decision-engine` project.
2.  Run the following command:

    ```bash
    make init-local-pg
    ```

    This command will:
    *   Start a PostgreSQL container.
    *   Run database migrations using the `db-migrator-postgres` service.
    *   Build the Decision Engine application from your local source code within a Docker container (`open-router-local-pg`).
    *   Start the newly built application container, connected to the PostgreSQL database.

This allows you to test your local code changes in an environment where PostgreSQL is managed by Docker.

### 3. Using Local Changes with a Local PostgreSQL Installation

This method is for developers who have PostgreSQL installed and running directly on their local machine (not in Docker).

**Steps:**

1.  **Ensure PostgreSQL is Running:**
    Make sure your local PostgreSQL server is running and accessible.

2.  **Set up Environment Variables (Optional but Recommended):**
    The application and `diesel` CLI use environment variables to connect to the database.

    The `Justfile` provides defaults if these are not set:
    *   `DB_USER` (default: `db_user`)
    *   `DB_PASSWORD` (default: `db_pass`)
    *   `DB_HOST` (default: `localhost`)
    *   `DB_PORT` (default: `5432`)
    *   `DB_NAME` (default: `decision_engine_db`)
    *   `DATABASE_URL` (derived default: `postgresql://db_user:db_pass@localhost:5432/decision_engine_db`)

    **Example of exporting variables in your shell (using default values):**
    ```bash
    export DB_USER="db_user"
    export DB_PASSWORD="db_pass"
    export DB_HOST="localhost"
    export DB_PORT="5432"
    export DB_NAME="decision_engine_db"
    # DATABASE_URL will be constructed by the application or Justfile if not set,
    ```
3.  **Drop Database if Exist:**
    
    ```bash
    just resurrect
    ```
    This command will drop the Database and create a new one.
4.  **Run Database Migrations:**
    Apply the database schema migrations to your local PostgreSQL database:

    ```bash
    just migrate-pg
    ```
    This command uses `diesel migration run` with the PostgreSQL specific migration directory (`migrations_pg`) and configuration file (`diesel_pg.toml`).

5.  **Run the Application:**
    Compile and run the Decision Engine application, ensuring it's built with PostgreSQL features:
    ```bash
    RUSTFLAGS="-Awarnings" cargo run --no-default-features --features postgres
    ```
    *   `RUSTFLAGS="-Awarnings"`: Suppresses warnings during compilation (optional).
    *   `--no-default-features --features postgres`: Ensures the application is compiled specifically for PostgreSQL, excluding default features (like MySQL support if it's a default) and including the `postgres` feature.

The application will start and connect to your local PostgreSQL database.

## Configuration

The database connection URL is typically configured in:

*   `config/development.toml` (for local cargo runs)
*   `config/docker-configuration.toml` (often mapped into Docker containers)

Ensure the `[database.url]` points to your PostgreSQL instance. For example:
`url = "postgresql://db_user:db_pass@localhost:5432/decision_engine_db"`

For Docker setups (`make init-pg`, `make init-local-pg`), the `docker-compose.yaml` file handles the service linking and environment variables to ensure the application container can connect to the PostgreSQL container (usually aliased as `postgres` or a similar hostname within the Docker network).

## Troubleshooting

*   **Connection Issues:**
    *   Verify `DATABASE_URL` is correct and the PostgreSQL server is accessible from where the application is running (your local machine or Docker container).
    *   Check PostgreSQL logs for any connection errors.
    *   Ensure firewalls are not blocking the connection.
*   **Migration Failures:**
    *   Check `diesel_pg.toml` for correct configuration.
    *   Ensure the migration files in `migrations_pg/` are correctly formatted.
    *   If you encounter issues with a "dirty" database state after a failed migration, you might need to manually resolve it in the database or use `diesel migration redo` (with caution).
*   **`just` command not found:**
    *   Install `just` by following its official installation guide.
*   **`diesel` command not found:**
    *   Install `diesel_cli` with PostgreSQL support: `cargo install diesel_cli --no-default-features --features postgres`.

This guide should help you get the Decision Engine up and running with PostgreSQL.


# Metrics

This document provides an overview of the metrics implementation in the routing layer.

## Overview

The metrics server is responsible for exposing key performance indicators of the application. It uses the `prometheus` crate to register and expose metrics in a format that can be scraped by a Prometheus server.

## How it works

The metrics server is built using `axum` and runs on a separate port from the main application server. It exposes a `/metrics` endpoint that returns the current state of all registered metrics.

The server is initialized in the `metrics_server_builder` function in `src/metrics.rs`. This function creates a new `axum` router and binds it to the address specified in the configuration.

## Available Metrics

The following metrics are exposed by the server:

### Counters

-   `api_requests_total`: A counter that tracks the total number of API requests received by the application. It has a single label, `endpoint`, which is the path of the API endpoint that was called.

-   `api_requests_by_status`: A counter that tracks the number of API requests grouped by endpoint and result status. It has two labels: `endpoint` and `status`.

### Histograms

-   `api_latency_seconds`: A histogram that measures the latency of API calls. It has a single label, `endpoint`, and uses exponential buckets to provide a detailed view of the latency distribution.

## How to check metrics

To check the metrics, you can hit the following endpoint:

```sh
curl http://127.0.0.1:9090/metrics
````

This will return a text-based representation of the current metrics, which can be ingested by a Prometheus server.
