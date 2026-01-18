# Build stage
FROM rust:1.84 as builder

WORKDIR /app

# Copy dependency manifests
COPY Cargo.toml ./
COPY src ./src

# Build for release
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    curl \
    python3 \
    python3-pip \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/hanzo-cloud /usr/local/bin/hanzo-cloud

# Copy migrations
COPY migrations ./migrations

# Create directory for experience libraries
RUN mkdir -p /app/experiences

# Expose port
EXPOSE 8001

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8001/health || exit 1

# Run the application
CMD ["hanzo-cloud"]
