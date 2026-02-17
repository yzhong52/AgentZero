#!/bin/bash
# Build and run Property Parser locally

set -e

echo "Installing frontend dependencies..."
npm --prefix frontend install

echo "Building React frontend..."
npm --prefix frontend run build

echo "Starting Rust server..."
cargo run --release --manifest-path backend/Cargo.toml
