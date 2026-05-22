FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:8ad7fc8691f916b45b537e22b3bbcf79b5978ed54088d23aff5742d27476346e

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
