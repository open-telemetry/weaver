# The build image
FROM --platform=$BUILDPLATFORM docker.io/rust:1.92.0@sha256:48851a839d6a67370c9dbe0e709bedc138e3e404b161c5233aedcf2b717366e4 AS weaver-build
WORKDIR /build
ARG BUILDPLATFORM
ARG TARGETPLATFORM

# Install Node.js for building UI
RUN curl -fsSL https://deb.nodesource.com/setup_24.x | bash - && \
    apt-get install -y nodejs

# list out directories to avoid pulling local cargo `target/`
COPY Cargo.toml /build/Cargo.toml
COPY Cargo.lock /build/Cargo.lock
COPY build.rs /build/build.rs
COPY .cargo /build/.cargo
COPY crates /build/crates
COPY data /build/data
COPY src /build/src
COPY tests /build/tests
COPY defaults /build/defaults
COPY schemas /build/schemas
COPY ui /build/ui
COPY cross-arch-build.sh /build/cross-arch-build.sh

# Build weaver
RUN ./cross-arch-build.sh

# The runtime image
FROM docker.io/alpine:3.23.0@sha256:51183f2cfa6320055da30872f211093f9ff1d3cf06f39a0bdb212314c5dc7375
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
