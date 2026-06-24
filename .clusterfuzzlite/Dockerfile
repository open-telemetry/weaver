FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:266156b055e6e6412a29570933d18084f182481854c8494f830e82fddaf35e10

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
