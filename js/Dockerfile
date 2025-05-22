FROM ubuntu:24.04 AS build

WORKDIR /build
ENV DEBIAN_FRONTEND=noninteractive
ENV PNPM_HOME="/root/.local/share/pnpm"
ENV PATH="$PNPM_HOME:$PATH"

# Install required tools
RUN apt-get update && apt-get install -y \
  curl \
  ca-certificates \
  gnupg \
  lsb-release \
  sudo \
  just \
  build-essential

# Add NodeSource PPA for latest Node.js
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && \
	apt-get install -y nodejs

# Install pnpm so it's cached between builds.
RUN corepack enable && corepack prepare pnpm@10.11.0 --activate

COPY . .

# Install deps with cache mount
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
    pnpm -r install --frozen-lockfile

# Run the CI checks and make sure it builds.
RUN just check --frozen-lockfile && \
	just build --frozen-lockfile

FROM scratch AS final
COPY --from=build /build/hang/dist /dist