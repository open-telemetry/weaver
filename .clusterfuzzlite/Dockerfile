FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:5fd6a6afd7369a3277b9697b39cb44bf8ed3f389bb440e7294a3f3869891ef97

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
