
# The build image
FROM rust:1.76.0-alpine3.18 as weaver-build
RUN apk add musl-dev
WORKDIR /build

# list out directories to avoid pulling local cargo `target/`
COPY Cargo.toml /build/Cargo.toml
COPY Cargo.lock /build/Cargo.lock
COPY crates /build/crates
COPY data /build/data
COPY src /build/src
COPY tests /build/tests
COPY defaults /build/defaults

# Build weaver
RUN cargo build --release

# The runtime image
FROM alpine:3.18.3
LABEL maintainer="The OpenTelemetry Authors"
RUN addgroup weaver \
  && adduser \
  --ingroup weaver \
  --no-create-home \
  --disabled-password \
  weaver
WORKDIR /weaver
COPY --from=weaver-build /build/target/release/weaver /weaver/weaver
USER weaver
ENTRYPOINT ["/weaver/weaver"]
