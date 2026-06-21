FROM gcr.io/oss-fuzz-base/base-builder-rust@sha256:82af3159e27830274364fa73a7d361ec5fa06bf376226eba0b28d807268ca103

COPY . $SRC/weaver
WORKDIR $SRC/weaver
COPY .clusterfuzzlite/build.sh $SRC/
