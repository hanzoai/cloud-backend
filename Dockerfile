# Build stage - using nightly for edition2024 support
FROM rustlang/rust:nightly as builder

# Allow PyO3 to work with Python 3.13+
ENV PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1

# Install Python development libraries for PyO3 linking
RUN apt-get update && apt-get install -y \
    python3-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy dependency manifests and source
COPY Cargo.toml ./
COPY src ./src
COPY migrations ./migrations

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
