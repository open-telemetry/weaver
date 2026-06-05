FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:9250f1239e9b3e4d3a8bd012b0a0d20937a952b25303ff8ba687a89ddec81205

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
