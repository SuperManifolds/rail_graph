# Build stage
FROM rust:1.83 AS builder

# Install trunk via cargo
RUN cargo install --locked trunk

# Add wasm target
RUN rustup target add wasm32-unknown-unknown

# Create app directory
WORKDIR /app

# Copy all source code
COPY . .

# Build the WASM application with trunk
RUN trunk build --release

# Build the static file server
RUN cargo build --release -p rail-graph-server

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r appuser && useradd -r -g appuser appuser

WORKDIR /app

# Copy built server binary
COPY --from=builder /app/target/release/rail-graph-server .

# Copy static files from trunk build
COPY --from=builder /app/dist ./dist

# Change ownership to non-root user
RUN chown -R appuser:appuser /app

# Switch to non-root user
USER appuser

# Expose port
EXPOSE 8080

# Set default PORT if not provided
ENV PORT=8080

# Run the static file server
CMD ["./rail-graph-server"]
