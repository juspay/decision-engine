# Decision Engine Setup Guide

## Overview

This guide provides detailed instructions for setting up and configuring the Decision Engine. The Decision Engine is a sophisticated payment routing system designed to optimize payment gateway selection in real-time for each transaction.

## Prerequisites

Before setting up the Decision Engine, ensure you have the following prerequisites:

1. **Docker**: The Decision Engine uses Docker for containerized deployment.
   - [Docker Installation for Mac](https://docs.docker.com/desktop/setup/install/mac-install/)
   - [Docker Installation for Windows](https://docs.docker.com/desktop/setup/install/windows-install/)
   - [Docker Installation for Linux](https://docs.docker.com/desktop/setup/install/linux-install/)

2. **Docker Compose**: Required for orchestrating multi-container Docker applications.
   - Usually included with Docker Desktop
   - [Standalone Installation](https://docs.docker.com/compose/install/)

3. **Git**: For cloning the repository.
   - [Git Installation](https://git-scm.com/book/en/v2/Getting-Started-Installing-Git)

4. **System Requirements**:
   - Approximately 2GB of disk space
   - At least 4GB of RAM (recommended)

## Installation Steps

### 1. Clone the Repository

```bash
git clone {repo-url}
cd {repo-directory}
```

Replace `{repo-url}` with the actual repository URL and `{repo-directory}` with the repository directory name.

### 2. First-Time Setup

For first-time setup, run the following command:

```bash
make init
```

This command performs the following operations:
- Sets up the environment
- Creates and configures the database with the required schema
- Sets up Redis for caching
- Starts the server for running the application
- Loads the routing configurations from `config.yaml` 
- Loads the priority logic rules from `priority_logic.txt`

### 3. Starting the Server (Without Resetting the Database)

If you've already completed the initial setup and don't want to reset the database, use:

```bash
make run
```

This command starts the server without running the database migrations.

### 4. Updating Configurations

After modifying the routing configurations in `config.yaml` or the priority logic rules in `priority_logic.txt`, run:

```bash
make update-config
```

This command pushes the updated configurations to the database.

### 5. Stopping the Server

To stop all running Docker containers:

```bash
make stop
```

### 6. Running with Local Code Changes

If you've made changes to the code locally and want to test them:

```bash
# Initialize the local environment
make init-local

# Run the local version
make run-local
```

## Configuration Files

### Main Configuration Files

1. **config.yaml**

This file contains the routing configurations for success rate-based routing and elimination thresholds. It is located in the `routing-config` directory:

```yaml
merchant_id: test_merchant1
priority_logic:
  script: priority_logic.txt
  tag: PL_TEST
elimination_config:
  threshold: 0.35
sr_routing_config:
  defaultBucketSize: 50
  defaultHedgingPercent: 5
  subLevelInputConfig:
    - paymentMethodType: UPI
      paymentMethod: UPI_COLLECT
      bucketSize: 100
      hedgingPercent: 1
    - paymentMethodType: UPI
      paymentMethod: UPI_PAY
      bucketSize: 500
      hedgingPercent: 1
    # Additional configurations...
```

2. **priority_logic.txt**

This file contains the Groovy script that defines the priority logic rules. It is located in the `routing-config` directory:

```groovy
def priorities = ['A','B','C','D','E'] // Default priorities if no rule matches
def systemtimemills = System.currentTimeMillis() % 100
def enforceFlag = false

if ((payment.paymentMethodType)=='UPI' && (txn.sourceObject)=='UPI_COLLECT') {
    priorities = ['A','B']
    enforceFlag = true
}
else {
    // Additional rule logic...
}
```

### Application Configuration

The application configuration is managed through TOML files in the `config` directory:

1. **development.toml**: Configuration for development environments
2. **docker-configuration.toml**: Configuration for Docker-based deployments

These files define settings for:
- Database connections
- Redis connections
- Server configurations
- Logging
- TLS settings (if enabled)

Example structure:

```toml
[server]
host = "0.0.0.0"
port = 8080

[database]
username = "postgres"
password = "postgres"
host = "localhost"
port = 5432
dbname = "open_router"
pool_size = 5

[redis]
host = "localhost"
port = 6379
pool_size = 5
namespace = "open_router"

[log]
level = "debug"
file = "/var/log/open-router/service.log"
```

## Directory Structure

Understanding the project directory structure is important for effective configuration:

```
decision-engine/
├── config/                  # Application configuration files
├── docs/                    # Documentation
├── migrations/              # Database migration files
├── routing-config/          # Routing configuration files
│   ├── config.yaml          # Main routing configuration
│   └── priority_logic.txt   # Priority logic rules
├── src/                     # Source code
├── Cargo.toml               # Rust project configuration
├── docker-compose.yaml      # Docker Compose configuration
├── Dockerfile               # Docker build instructions
└── Makefile                 # Build and run commands
```

## Environment Variables

The Decision Engine can be configured using environment variables. These override the settings in the configuration files:

```bash
# Example environment variables
DATABASE_URL=postgres://user:password@host:port/dbname
REDIS_URL=redis://host:port
LOG_LEVEL=info
SERVER_PORT=8080
```

## Custom Routing Configuration

### Success Rate Routing Configuration

The `sr_routing_config` section in `config.yaml` defines how success rate-based routing works:

- `defaultBucketSize`: Number of recent transactions to consider for success rate calculation
- `defaultHedgingPercent`: Percentage of transactions to route to non-optimal gateways for exploration
- `subLevelInputConfig`: Configurations for specific payment method types and methods:
  - `paymentMethodType`: Type of payment method (e.g., UPI, CARD)
  - `paymentMethod`: Specific method within the type (e.g., UPI_PAY)
  - `bucketSize`: Custom bucket size for this specific method
  - `hedgingPercent`: Custom hedging percentage for this method

### Priority Logic Configuration

The `priority_logic` section references the Groovy script file (`priority_logic.txt`) that contains the rule definitions:

- The script defines the `priorities` array, which determines the order of gateway selection
- It can set the `enforceFlag` to true to strictly enforce the priority list
- It evaluates transaction details like payment method type, amount, and other parameters
- Based on the evaluation, it sets the appropriate priority order

## Testing the Setup

After completing the setup, you can test the Decision Engine using the provided API endpoints:

### 1. Gateway Decision API

```bash
curl --location 'http://localhost:8080/decide-gateway' \
--header 'Content-Type: application/json' \
--data '{
    "merchantId": "test_merchant1",
    "eligibleGatewayList": ["PAYU", "RAZORPAY", "PAYTM_V2"],
    "rankingAlgorithm": "SR_BASED_ROUTING",
    "eliminationEnabled": true,
    "paymentInfo": {
        "paymentId": "PAY12345",
        "amount": 100.50,
        "currency": "USD",
        "customerId": "CUST12345",
        "paymentType": "ORDER_PAYMENT",
        "paymentMethodType": "UPI",
        "paymentMethod": "UPI_PAY"
    }
}'
```

### 2. Update Gateway Score API

```bash
curl --location 'http://localhost:8080/update-gateway-score' \
--header 'Content-Type: application/json' \
--data '{
    "merchantId": "test_merchant1",
    "gateway": "PAYU",
    "gatewayReferenceId": null,
    "status": "CHARGED",
    "paymentId": "PAY12345"
}'
```

## Troubleshooting

### Common Issues

1. **Database Connection Failures**:
   - Verify database credentials in the configuration files
   - Ensure the database server is running
   - Check network connectivity to the database server

2. **Redis Connection Failures**:
   - Verify Redis configuration in the configuration files
   - Ensure the Redis server is running
   - Check network connectivity to the Redis server

3. **Docker-related Issues**:
   - Ensure Docker and Docker Compose are properly installed
   - Check if Docker services are running (`docker ps`)
   - Verify that the required ports are available

4. **Configuration Loading Issues**:
   - Check syntax in TOML configuration files
   - Ensure YAML and Groovy scripts are correctly formatted
   - Verify file paths and permissions

### Checking Logs

To view the logs for debugging:

```bash
# View Docker container logs
docker logs <container-id>

# Follow logs in real-time
docker logs -f <container-id>
```

### Resetting the Environment

If you encounter persistent issues, you can reset the environment:

```bash
# Stop all containers
make stop

# Remove Docker volumes (will delete all data)
docker-compose down -v

# Reinitialize the environment
make init
```

## Next Steps

After successfully setting up the Decision Engine, consider:

1. **Custom Configuration**: Tailor the routing rules to your specific business requirements
2. **Integration Testing**: Test the Decision Engine with your payment orchestrator
3. **Monitoring Setup**: Implement monitoring and alerting for the Decision Engine
4. **Performance Tuning**: Optimize the configuration for your transaction volume
