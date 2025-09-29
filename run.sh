#!/bin/bash

# Install trunk if not already installed
if ! command -v trunk &> /dev/null; then
    echo "Installing trunk..."
    cargo install trunk
fi

# Install wasm32 target if not already installed
rustup target add wasm32-unknown-unknown

# Run the development server
echo "Starting development server at http://127.0.0.1:8080"
trunk serve --open