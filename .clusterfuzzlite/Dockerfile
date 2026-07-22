FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:8822b56a21478614ed181d183683451e4f7b959d0a34043d95fc8b9f7fcc85f9

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
