#!/bin/bash

# Script para build y deploy en AWS Lambda
set -e
echo "🔨 Building Rust project for Lambda..."

# Install cargo-lambda if not present
if ! command -v cargo-lambda &> /dev/null; then
    echo "📦 Installing cargo-lambda..."
    cargo install cargo-lambda
fi

# Build for Lambda
cargo lambda build --release

echo "📦 Building SAM package..."

# Build SAM package
sam build

echo "🚀 Deploying to AWS..."

# Deploy
sam deploy --guided

echo "✅ Deployment completed!"
echo ""
echo "🌐 Your API is now available at the URL shown above."
echo "💡 Test with: curl -X POST [API_URL] -H 'Content-Type: application/json' -d '{\"image_data\": \"[base64_image]\"}'"