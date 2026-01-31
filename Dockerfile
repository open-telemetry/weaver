# The build image
FROM --platform=$BUILDPLATFORM docker.io/rust:1.93.0@sha256:4c7eb947d7e078f5c076e086c7b75c36ea0ec7c685f2244b3d79306deb7e44b7 AS weaver-build
WORKDIR /build
ARG BUILDPLATFORM
ARG TARGETPLATFORM

# Install Node.js for building UI
# renovate: datasource=node-version depName=node
ARG NODE_VERSION=24
RUN curl -fsSL https://deb.nodesource.com/setup_${NODE_VERSION}.x | bash - && \
  apt-get install -y nodejs

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
