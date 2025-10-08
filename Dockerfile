# Dockerfile for Nimby Graph production deployment

FROM rust:1.75

# Install trunk and wasm target
RUN cargo install --locked trunk
RUN rustup target add wasm32-unknown-unknown

# Create app directory
WORKDIR /app

# Copy application files
COPY . .

# Build the application in release mode
RUN trunk build --release

# Expose port
EXPOSE 8080

# Run trunk serve in release mode
CMD ["trunk", "serve", "--release", "--address", "0.0.0.0", "--port", "8080"]
