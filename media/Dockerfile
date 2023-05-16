# Create a build image
FROM ubuntu:latest

# Install necessary packages
RUN apt-get update && \
	apt-get install -y \
	ca-certificates \
	wget \
	ffmpeg

# Create a media volume
VOLUME /media
WORKDIR /media

# Download a file from the internet, in this case my boy big buck bunny
RUN wget http://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4 -O source.mp4

# Copy an run a script to create a fragmented mp4 (more overhead, easier to split)
COPY fragment .
CMD ./fragment
