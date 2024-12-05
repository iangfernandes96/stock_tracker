# Use the official Rust image as the base image
FROM rust:1.70 as builder

# Set the working directory
WORKDIR /usr/src/stock_tracker

# Copy the Cargo.toml and Cargo.lock files
COPY Cargo.toml Cargo.lock ./

# # Create a new empty shell project
# RUN cargo init

# Copy the source code
COPY src ./src

# Build the application
RUN cargo build --release

# Use a smaller base image for the final image
FROM debian:buster-slim

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/stock_tracker/target/release/stock_tracker /usr/local/bin/stock_tracker

# Expose the application port
EXPOSE 3030

# Command to run the application
CMD ["stock_tracker"]