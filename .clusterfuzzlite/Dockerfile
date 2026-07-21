FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:a427e739824ab8b42e626648e8f3f621b01173fc23272e7451f70e76fc89bfe3

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
