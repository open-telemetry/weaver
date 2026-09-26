FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:bc7d32d2a86bbe8afa49fdaa1a7665d17125cae20bfdc581e5335ef97aebcd2a

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
