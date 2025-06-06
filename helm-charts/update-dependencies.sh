#!/bin/bash
set -e

echo "Updating Decision Engine Helm chart dependencies..."

# Add Bitnami repo for dependencies
echo "Adding Bitnami repository..."
helm repo add bitnami https://charts.bitnami.com/bitnami
helm repo update

# Ensure charts directory exists
echo "Creating charts directory if it doesn't exist..."
mkdir -p charts

# Download dependencies explicitly
echo "Downloading PostgreSQL chart..."
helm pull bitnami/postgresql --version ~12.5.5 --untar --untardir charts

echo "Downloading Redis chart..."
helm pull bitnami/redis --version ~17.11.3 --untar --untardir charts

# Build dependencies
echo "Building dependencies..."
helm dependency build

echo "Dependencies updated successfully!"
echo ""
echo "The following dependencies have been downloaded:"
ls -la charts/

echo ""
echo "To install the chart, run:"
echo "helm install my-release ."
