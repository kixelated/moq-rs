# Use the official Rust image as the base image
FROM rust:latest as build

# Quiche requires docker
RUN apt-get update && \
    apt-get install -y cmake

# Set the build directory
WORKDIR /warp

# Create an empty project
RUN cargo init --bin

# Copy the Cargo.toml and Cargo.lock files to the container
COPY Cargo.toml Cargo.lock ./

# Build the empty project so we download/cache dependencies
RUN cargo build --release

# Copy the entire project to the container
COPY . .

# Build the project
RUN cargo build --release

# Make a new image to run the binary
FROM ubuntu:latest

# Use a volume to access certificates
VOLUME /cert

# Use another volume to access the media
VOLUME /media

# Expose port 4443 for the server
EXPOSE 4443/udp

# Copy the built binary
COPY --from=build /warp/target/release/warp /bin

# Set the startup command to run the binary
CMD warp --cert /cert/localhost.crt --key /cert/localhost.key --media /media/fragmented.mp4