#!/bin/bash
set -e

# Default release name
RELEASE_NAME="my-release"
VALUES_FILE=""

# Parse command-line arguments
while [[ $# -gt 0 ]]; do
  key="$1"
  case $key in
    -n|--name)
      RELEASE_NAME="$2"
      shift
      shift
      ;;
    -f|--values)
      VALUES_FILE="$2"
      shift
      shift
      ;;
    -h|--help)
      echo "Usage: ./install.sh [-n|--name RELEASE_NAME] [-f|--values VALUES_FILE]"
      echo ""
      echo "Options:"
      echo "  -n, --name RELEASE_NAME   Specify release name (default: my-release)"
      echo "  -f, --values VALUES_FILE  Specify values file (optional)"
      echo "  -h, --help                Show this help message"
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      echo "Use --help for usage information."
      exit 1
      ;;
  esac
done

echo "Installing Decision Engine Helm chart..."

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

# Install the chart
if [ -n "$VALUES_FILE" ]; then
  echo "Installing chart with release name '$RELEASE_NAME' using values from '$VALUES_FILE'..."
  helm install $RELEASE_NAME . -f $VALUES_FILE
else
  echo "Installing chart with release name '$RELEASE_NAME' using default values..."
  helm install $RELEASE_NAME .
fi

echo ""
echo "Decision Engine has been installed!"
echo "Run 'kubectl get pods' to check the status of the pods."
