FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:04a8af7ad9a5daa100629bcde77fb0c5c83b40b72d026d0fbcd9e129ae026eaf

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
