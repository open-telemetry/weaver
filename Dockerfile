# The build image
FROM --platform=$BUILDPLATFORM docker.io/rust:1.92.0@sha256:48851a839d6a67370c9dbe0e709bedc138e3e404b161c5233aedcf2b717366e4 AS weaver-build
WORKDIR /build
ARG BUILDPLATFORM
ARG TARGETPLATFORM

# list out directories to avoid pulling local cargo `target/`
COPY Cargo.toml /build/Cargo.toml
COPY Cargo.lock /build/Cargo.lock
COPY .cargo /build/.cargo
COPY crates /build/crates
COPY data /build/data
COPY src /build/src
COPY tests /build/tests
COPY defaults /build/defaults
COPY cross-arch-build.sh /build/cross-arch-build.sh

# Build weaver
RUN ./cross-arch-build.sh

# The runtime image
FROM docker.io/alpine:3.23.2@sha256:c93cec902b6a0c6ef3b5ab7c65ea36beada05ec1205664a4131d9e8ea13e405d
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
