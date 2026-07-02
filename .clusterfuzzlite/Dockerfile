FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:844cb1b78ea855a2c52db10319e643bfb3ac240bedbd2a5ee0f4bcf3b611a913

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
