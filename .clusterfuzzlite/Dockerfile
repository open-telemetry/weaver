FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:73be59e48262b7243f815f1c981a6e753e95e4b13e4d085cc9fc06bd79d42daa

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
