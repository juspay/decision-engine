#!/bin/bash
set -e

# Step 1: Setup virtualenv and install Python deps
echo "🌱 Creating virtual environment..."
python3 -m venv venv
source venv/bin/activate

echo "📦 Installing Python dependencies..."
pip install -r requirements.txt

# Step 2: Run merchant configuration logic
echo "🚀 Running merchant SQL config script..."
python setup.py
