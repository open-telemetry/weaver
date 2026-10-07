FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:412929965850a4c232f4191fece4f44d6726de53249698e95e5d4134117f38e4

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
