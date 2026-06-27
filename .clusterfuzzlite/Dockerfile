FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:58e9c2999e726427d67cd9bdab7267ee6f1d0b4dc8326236e8fe8ea16795b3ba

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
