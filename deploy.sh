#!/bin/bash
# Exit immediately if a command exits with a non-zero status
set -e

echo "=========================================="
echo "Starting deploy: building release..."
echo "=========================================="

# Build release artifacts using trunk
trunk build --release

echo "=========================================="
echo "Publishing to GitHub Pages..."
echo "=========================================="

# Deploy to GitHub Pages
npx gh-pages -d dist

echo "=========================================="
echo "Deployment successful!"
echo "=========================================="
