FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:da3e108478a5f0ec57de4de1a92c4ef97c12900bcc6034c236dc4d9a4fbbb5ab

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
