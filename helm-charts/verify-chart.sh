#!/bin/bash
set -e

echo "Verifying Decision Engine Helm chart..."

# Add Bitnami repo for dependencies
echo "Adding Bitnami repository..."
helm repo add bitnami https://charts.bitnami.com/bitnami
helm repo update

# Lint the chart
echo "Linting the chart..."
helm lint .

# Template the chart to validate templates
echo "Validating templates..."
helm template test .

# Verify chart dependencies
echo "Checking dependencies..."
helm dependency update .

echo "Chart verification completed successfully!"
echo "To install the chart, run:"
echo "helm install my-release ."
