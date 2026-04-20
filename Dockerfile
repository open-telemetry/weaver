# The build image
FROM docker.io/rust:1.95.0@sha256:0ccf3b8640fa428d4ff623644960010144fbadd3523d45e66374de1ca2e7abf6 AS weaver-build
WORKDIR /build

# Install Node.js and musl build dependencies
# renovate: datasource=node-version depName=node
ARG NODE_VERSION=24
RUN curl -fsSL https://deb.nodesource.com/setup_${NODE_VERSION}.x | bash - && \
  apt-get install -y nodejs musl-tools musl-dev perl

# Copy UI package files first for better layer caching
RUN npm install -g pnpm
COPY ui/package.json ui/pnpm-lock.yaml /build/ui/
RUN cd /build/ui && pnpm install --frozen-lockfile

# Copy UI source files
COPY ui /build/ui

# Copy Rust dependencies for better layer caching
COPY Cargo.toml Cargo.lock build.rs /build/
COPY .cargo /build/.cargo

# Copy source files
COPY crates /build/crates
COPY data /build/data
COPY src /build/src
COPY tests /build/tests
COPY defaults /build/defaults
COPY cross-arch-build.sh /build/cross-arch-build.sh

# Build the UI
RUN cd /build/ui && pnpm build

# Build weaver
RUN ./cross-arch-build.sh

# The runtime image
FROM docker.io/alpine:3.23.4@sha256:5b10f432ef3da1b8d4c7eb6c487f2f5a8f096bc91145e68878dd4a5019afde11
LABEL maintainer="The OpenTelemetry Authors"
RUN addgroup weaver \
  && adduser \
  --ingroup weaver \
  --disabled-password \
  weaver
WORKDIR /home/weaver
COPY --from=weaver-build --chown=weaver:weaver /build/weaver /weaver/weaver
USER weaver
RUN mkdir /home/weaver/target
ENTRYPOINT ["/weaver/weaver"]
