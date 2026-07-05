FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:f3912472e3d2e8a645cc8911a71d2ba13f1a37fb69796cbea2bc22ebf5d81b4a

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
