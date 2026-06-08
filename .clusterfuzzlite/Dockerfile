FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:44c906bab39d3c75733008b0fe30356920b1bf165df0dbbf03f37d8136b0f696

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
