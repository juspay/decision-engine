#!/bin/sh

echo 'Waiting for ClickHouse to be ready...'
sleep 10

echo 'Running analytics migrations...'
for file in /analytics/migrations/*.sql; do
    if [ -f "$file" ]; then
        echo "Executing: $file"
        clickhouse-client --host clickhouse --port 9000 --user analytics_user --password analytics_pass --multiquery --query "$(cat $file)"
        if [ $? -eq 0 ]; then
            echo "Successfully executed: $file"
        else
            echo "Failed to execute: $file"
            exit 1
        fi
    fi
done

echo 'Analytics migrations completed!'
