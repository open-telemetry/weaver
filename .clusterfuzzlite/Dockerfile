FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:f1874a7702b672b01d996cbbe07fd3e0dab22e5bb99acc74f9940a1672456a02

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
