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
else
  echo "Unsupported target platform: ${TARGETPLATFORM}"
  exit 1
fi

# Fix for aws-lc-sys cross-compilation to musl:
# glibc 2.38+ redirects strtol/sscanf to __isoc23_* variants when C23 features are enabled.
# These symbols don't exist in musl, causing linker errors.
# Disable the C23 function redirects by unsetting __GLIBC_USE_ISOC2X.
export AWS_LC_SYS_CFLAGS="-U__GLIBC_USE_ISOC2X -D__GLIBC_USE_ISOC2X=0"

rustup target add "${RUST_TARGET}"
cargo build --release --target="${RUST_TARGET}"
cp "target/${RUST_TARGET}/release/weaver" .
