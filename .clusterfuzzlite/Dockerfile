FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:8a5b2b370c7778ce0190986e487dde952aa1d80496721f5647a3f11838a00a1c

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
