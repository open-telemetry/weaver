
# The build image
FROM rust:1.76.0-alpine3.18 as weaver-build
RUN apk add musl-dev
WORKDIR /build
COPY . /build
RUN cargo build --release

# The runtime image
FROM alpine:3.18.3
LABEL maintainer="The OpenTelemetry Authors"
WORKDIR /weaver
COPY --from=weaver-build /build/target/release/weaver /weaver/weaver
ENTRYPOINT ["/weaver/weaver"]