FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:53c885625d133bff8a11b553f2098127f6bc92b4a1c6c23130c76cad8218b1c1

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
