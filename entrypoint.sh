#!/bin/sh

# Set default binary if APP_BINARY is not provided
APP_BINARY=${APP_BINARY:-dynamo}

# Path to the binary
BINARY_PATH="/app/bin/$APP_BINARY"

# Check if the binary exists
if [ ! -f "$BINARY_PATH" ]; then
    echo "Error: Binary '$APP_BINARY' not found in /app/bin"
    exit 1
fi

echo "Starting: $APP_BINARY"
exec "$BINARY_PATH"
