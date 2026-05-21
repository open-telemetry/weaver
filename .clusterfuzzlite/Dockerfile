FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:324fd72a7251a327a1757d0fc6a8edb13a24dfce819e5f7df5b41c1c6df0d229

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
