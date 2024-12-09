# Build stage
FROM rust:1.82 AS builder

WORKDIR /usr/src/app

# Create a new empty shell project
RUN cargo new --bin investigate-jobs
WORKDIR /usr/src/app/investigate-jobs

# Copy manifests for dependency caching
COPY Cargo.* ./

# Cache dependencies
RUN cargo build --release
RUN rm src/*.rs

# Copy source code
COPY . .

# Build for release
RUN rm ./target/release/deps/investigate_jobs*
RUN cargo build --release

# Final stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/investigate-jobs/target/release/investigate-jobs /usr/local/bin/app

CMD ["app"] 
