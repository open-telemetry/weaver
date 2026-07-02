# The build image
FROM docker.io/rust:1.96.1@sha256:1f0dbad1df66647807e6952d1db85d0b2bda7606cb2139d82517e4f009967376 AS weaver-build
WORKDIR /build

# Install Node.js and musl build dependencies
# renovate: datasource=node-version depName=node
ARG NODE_VERSION=24
RUN curl -fsSL https://deb.nodesource.com/setup_${NODE_VERSION}.x -o /tmp/nodesource-setup.sh && \
  echo "6e3d580f5bd7ccf2aa1e8df8d35c60d78e873c3ff8beb282c9bebd914904ad72  /tmp/nodesource-setup.sh" | sha256sum -c && \
  bash /tmp/nodesource-setup.sh && \
  apt-get install -y nodejs musl-tools musl-dev perl

# Copy UI package files first for better layer caching
COPY ui/package.json ui/pnpm-lock.yaml /build/ui/
# Use Corepack to provision the hash-pinned pnpm declared in ui/package.json's
# "packageManager" field; Corepack verifies the download against that integrity hash.
RUN corepack enable
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
FROM docker.io/alpine:3.24.1@sha256:28bd5fe8b56d1bd048e5babf5b10710ebe0bae67db86916198a6eec434943f8b
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
