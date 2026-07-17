FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:c0d09a5cca5bcf59da14922d0f910d3f7045f12ee44a956778cbcd33c4645f35

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
