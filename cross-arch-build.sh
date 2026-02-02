#!/bin/sh
set -exu

# Detect host architecture for native musl builds
HOST_ARCH=$(uname -m)
echo "Host architecture: ${HOST_ARCH}"

case "${HOST_ARCH}" in
  x86_64)
    RUST_TARGET=x86_64-unknown-linux-musl
    export CC_x86_64_unknown_linux_musl=musl-gcc
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc
    ;;
  aarch64)
    RUST_TARGET=aarch64-unknown-linux-musl
    export CC_aarch64_unknown_linux_musl=musl-gcc
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc
    ;;
  *)
    echo "Unsupported architecture: ${HOST_ARCH}"
    exit 1
    ;;
esac

rustup target add "${RUST_TARGET}"
cargo build --release --target="${RUST_TARGET}"
cp "target/${RUST_TARGET}/release/weaver" .
