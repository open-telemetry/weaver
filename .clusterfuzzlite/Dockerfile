FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:69eb410c0be09f76a2e82609a47b79abb970ea2535e478dafe29a7246bff0c2c

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
