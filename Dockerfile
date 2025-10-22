# Dockerfile for RailGraph production deployment

FROM rust:1.83

# Install trunk via cargo
RUN cargo install --locked trunk

# Add wasm target
RUN rustup target add wasm32-unknown-unknown

# Create app directory
WORKDIR /app

# Copy manifests first for better caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo "pub fn add(a: i32, b: i32) -> i32 { a + b }" > src/lib.rs && \
    mkdir benches && \
    echo "fn main() {}" > benches/conflict_detection.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release --target wasm32-unknown-unknown

# Remove dummy source
RUN rm -rf src benches

# Copy real source code
COPY . .

# Touch source files to trigger rebuild (dependencies are cached)
RUN touch src/lib.rs

# Build the application in release mode
RUN trunk build --release

# Expose port
EXPOSE 8080

# Set default PORT if not provided
ENV PORT=8080

# Run trunk serve in release mode with configurable port
CMD sh -c "trunk serve --release --address 0.0.0.0 --port ${PORT}"
