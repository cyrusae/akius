#!/bin/bash
# Exit immediately if a command exits with a non-zero status
set -e

echo "=========================================="
echo "Starting deploy: building release..."
echo "=========================================="

# Clean previous build artifacts to prevent build pipeline cache errors
trunk clean

# Build release artifacts using trunk with public-url set to the repo subdirectory
trunk build --release --public-url "/akius/"

echo "=========================================="
echo "Publishing to GitHub Pages..."
echo "=========================================="

# Deploy to GitHub Pages
npx gh-pages -d dist

echo "=========================================="
echo "Deployment successful!"
echo "=========================================="
