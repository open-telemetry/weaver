set -exu

if [ "${TARGETPLATFORM}" = "linux/amd64" ]; then
  RUST_TARGET=x86_64-unknown-linux-musl
  if [ "${TARGETPLATFORM}" != "${BUILDPLATFORM}" ]; then
    apt-get update && apt-get install -y gcc-x86-64-linux-gnu
  fi
elif [ "${TARGETPLATFORM}" = "linux/arm64" ]; then
  RUST_TARGET=aarch64-unknown-linux-musl
  if [ "${TARGETPLATFORM}" != "${BUILDPLATFORM}" ]; then
    apt-get update && apt-get install -y gcc-aarch64-linux-gnu
  fi
elif [ "${TARGETPLATFORM}" = "darwin/arm64" ]; then
  RUST_TARGET=aarch64-apple-darwin
else
  echo "Unsupported target platform: ${TARGETPLATFORM}"
  exit 1
fi

rustup target add "${RUST_TARGET}"
cargo build --release --target="${RUST_TARGET}"
cp "target/${RUST_TARGET}/release/weaver" .
