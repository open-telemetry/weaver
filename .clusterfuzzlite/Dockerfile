FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:8a9f7d1ee933da04fc7b616c51d3fb99009692f6aa93ca9e972a6a39730e14f5

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
