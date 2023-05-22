# Use the official Node.js image as the build image
FROM node:latest

# Set the build directory
WORKDIR /build

# Copy the package.json and yarn.lock files to the container
COPY package*.json yarn.lock ./

# Install dependencies
RUN yarn install

# Copy the entire project to the container
COPY . .

# Expose port 4444 for serving the project
EXPOSE 4444

# Copy the certificate hash before running
VOLUME /cert

# Make a symlink to the certificate fingerprint
RUN ln -s /cert/localhost.hex fingerprint.hex

# Copy the certificate fingerprint and start the web server
CMD yarn parcel serve --https --cert /cert/localhost.crt --key /cert/localhost.key --port 4444