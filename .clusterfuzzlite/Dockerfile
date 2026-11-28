FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:53035c6f510f5f949828ab2cb70f551c3c37e43d94c26579228c9f1fa5fc6a1e

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
