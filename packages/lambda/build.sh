#!/bin/bash
set -e

echo "Building Rust Lambda function for ARM64..."

# Check if cargo-lambda is installed
if ! command -v cargo-lambda &> /dev/null; then
    echo "cargo-lambda not found."
    echo ""
    echo "Please install it with:"
    echo "  cargo install cargo-lambda"
    echo ""
    exit 1
fi

# Build using cargo-lambda
cargo lambda build --release --arm64

echo ""
echo "✅ Build complete!"
echo "Lambda deployment package ready at: target/lambda"
echo ""
echo "Binary size:"
ls -lh target/lambda/*/bootstrap
