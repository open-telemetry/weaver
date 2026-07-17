FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:e916d1cb7c6a7062cdd3851f216d4ecce7a6720da5f1f1964f90802bf25081c4

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
