#!/bin/bash
set -e
echo "🔨 Building y Deploying Rust project for Lambda..."

if ! command -v cargo-lambda &> /dev/null; then
    echo "📦 Installing cargo-lambda..."
    cargo install cargo-lambda
fi

echo "📦 Building SAM package..."

sam build

echo "🚀 Deploying to AWS..."

sam deploy

echo "✅ Deployment completed!"