# Stage 1: Build
FROM rust:bullseye as builder
# Set the working directory inside the container
WORKDIR /usr/src/leetcode_bot
COPY . .
# Build the application in release mode
RUN cargo build --release
# Stage 2: Runtime
FROM debian:bullseye-slim
RUN apt update && apt install -y ca-certificates curl
# Create a user to run the application
RUN useradd -m appuser
# Copy the built application from the builder stage
COPY --from=builder /usr/src/leetcode_bot/target/release/leetcode_bot /usr/local/bin/leetcode_bot
# Change ownership to the appuser
RUN chown appuser:appuser /usr/local/bin/leetcode_bot
# Switch to the appuser
USER appuser
# Run the application
CMD ["leetcode_bot"]
