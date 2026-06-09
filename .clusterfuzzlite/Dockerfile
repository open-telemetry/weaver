FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:dd7ae83552d3c91fbdaa3198d85b89701687cd69ecacfbd1970669bf2adf4744

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
