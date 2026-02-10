# The build image
FROM docker.io/rust:1.93.0@sha256:bbde3ca426faeebab3eb58b8005d3d6da70607817a14bb92a55c94611d4d905f AS weaver-build
WORKDIR /build

# Install Node.js and musl build dependencies
# renovate: datasource=node-version depName=node
ARG NODE_VERSION=24
RUN curl -fsSL https://deb.nodesource.com/setup_${NODE_VERSION}.x | bash - && \
  apt-get install -y nodejs musl-tools musl-dev perl

# Copy UI package files first for better layer caching
COPY ui/package.json ui/package-lock.json /build/ui/
RUN cd /build/ui && npm ci

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

# Build weaver
RUN ./cross-arch-build.sh

# The runtime image
FROM docker.io/alpine:3.23.3@sha256:25109184c71bdad752c8312a8623239686a9a2071e8825f20acb8f2198c3f659
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
