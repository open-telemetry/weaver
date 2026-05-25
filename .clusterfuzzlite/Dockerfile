FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:fb9bd152e6c1c22555a428dfbd77bdb412b710c602aab93f04fc854183ebd45d

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
