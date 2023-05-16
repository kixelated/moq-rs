# Use ubuntu because it's ez
FROM ubuntu:latest

# Use openssl and golang to generate certificates
RUN apt-get update && \
	apt-get install -y ca-certificates openssl golang

# Save the certificates to a volume
VOLUME /cert
WORKDIR /cert

# Download the go modules
COPY go.mod go.sum ./
RUN go mod download

# Copy over the remaining files.
COPY . .

# TODO support an output directory
CMD ./generate