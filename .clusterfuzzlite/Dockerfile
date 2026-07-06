FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:237ebdb74322706e3ab1d3449564b8096b6ed344415558578aa2477d9fb19cbc

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
