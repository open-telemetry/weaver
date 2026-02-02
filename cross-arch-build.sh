#!/bin/sh
set -exu

# Install build dependencies
apt-get update && apt-get install -y musl-tools musl-dev perl

# Detect actual host architecture (not Docker platform vars which may differ with QEMU)
HOST_ARCH=$(uname -m)
echo "Host architecture: ${HOST_ARCH}"
echo "Build platform: ${BUILDPLATFORM}"
echo "Target platform: ${TARGETPLATFORM}"

case "${TARGETPLATFORM}" in
  linux/amd64)
    RUST_TARGET=x86_64-unknown-linux-musl
    if [ "${HOST_ARCH}" = "x86_64" ]; then
      # Native x86_64 - musl-tools provides musl-gcc
      export CC_x86_64_unknown_linux_musl=musl-gcc
      export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc
    else
      # Cross-compiling to x86_64 from another arch
      apt-get install -y wget
      wget -q https://musl.cc/x86_64-linux-musl-cross.tgz
      tar xf x86_64-linux-musl-cross.tgz -C /opt
      export PATH="/opt/x86_64-linux-musl-cross/bin:$PATH"
      export CC_x86_64_unknown_linux_musl=x86_64-linux-musl-gcc
      export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc
    fi
    ;;
  linux/arm64*)
    RUST_TARGET=aarch64-unknown-linux-musl
    if [ "${HOST_ARCH}" = "aarch64" ]; then
      # Native arm64 - musl-tools provides musl-gcc
      export CC_aarch64_unknown_linux_musl=musl-gcc
      export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=musl-gcc
    else
      # Cross-compiling to arm64 from x86_64
      apt-get install -y wget
      wget -q https://musl.cc/aarch64-linux-musl-cross.tgz
      tar xf aarch64-linux-musl-cross.tgz -C /opt
      export PATH="/opt/aarch64-linux-musl-cross/bin:$PATH"
      export CC_aarch64_unknown_linux_musl=aarch64-linux-musl-gcc
      export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-musl-gcc
    fi
    ;;
  *)
    echo "Unsupported target platform: ${TARGETPLATFORM}"
    exit 1
    ;;
esac

rustup target add "${RUST_TARGET}"
cargo build --release --target="${RUST_TARGET}"
cp "target/${RUST_TARGET}/release/weaver" .
