FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:c62b4a41865adec0ffd93e9d21be7654a8359f75dee2e96a409325a494e93040

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
