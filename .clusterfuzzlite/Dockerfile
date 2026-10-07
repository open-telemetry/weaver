FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:47bbe5147371187603326af515511e6bad2dcd6e6912fa42c66fb161b5bd770b

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
