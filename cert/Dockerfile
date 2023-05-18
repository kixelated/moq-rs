# Use ubuntu because it's ez
FROM ubuntu:latest

WORKDIR /build

# Use openssl and golang to generate certificates
RUN apt-get update && \
	apt-get install -y ca-certificates openssl golang xxd


# Download the go modules
COPY go.mod go.sum ./
RUN go mod download

# Copy over the remaining files.
COPY . .

# Save the certificates to a volume
VOLUME /cert

# TODO support an output directory
CMD ./generate && cp localhost.* /cert