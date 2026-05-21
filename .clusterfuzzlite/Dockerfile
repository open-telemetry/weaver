FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:80e7586e4ab6780639e2eaa1eba34a804b34330cd929a6003dcf50db4afde6aa

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
