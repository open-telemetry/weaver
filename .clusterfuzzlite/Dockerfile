FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:5956ed11b95acdfe1e21399e3b067ce33140ac1e9681d1cd5b05b2a593d76b61

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
