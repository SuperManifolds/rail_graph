# Dockerfile for Nimby Graph production deployment

FROM rust:1.83

# Install trunk from binary release (much faster than cargo install)
RUN wget -qO- https://github.com/trunk-rs/trunk/releases/download/v0.21.4/trunk-x86_64-unknown-linux-gnu.tar.gz | tar -xzf- && \
    mv trunk /usr/local/bin/

# Add wasm target
RUN rustup target add wasm32-unknown-unknown

# Create app directory
WORKDIR /app

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn add(a: i32, b: i32) -> i32 { a + b }" > src/lib.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release --target wasm32-unknown-unknown

# Remove dummy source
RUN rm -rf src

# Copy real source code
COPY . .

# Touch source files to trigger rebuild (dependencies are cached)
RUN touch src/lib.rs

# Build the application in release mode
RUN trunk build --release

# Expose port
EXPOSE 8080

# Run trunk serve in release mode
CMD ["trunk", "serve", "--release", "--address", "0.0.0.0", "--port", "8080"]
