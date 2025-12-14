# The build image
FROM --platform=$BUILDPLATFORM docker.io/rust:1.91.1@sha256:4a29b0db5c961cd530f39276ece3eb6e66925b59599324c8c19723b72a423615 AS weaver-build
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
FROM docker.io/alpine:3.22.2@sha256:4b7ce07002c69e8f3d704a9c5d6fd3053be500b7f1c69fc0d80990c2ad8dd412
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
