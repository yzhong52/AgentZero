#!/bin/bash
# Build and run Property Parser locally

set -e

echo "📦 Installing frontend dependencies..."
npm install

echo "🔨 Building TypeScript frontend..."
npm run build

echo "🚀 Starting Rust server..."
cargo run --release
