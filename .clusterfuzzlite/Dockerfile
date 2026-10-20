FROM gcr.io/oss-fuzz-base/base-builder-rust:v1@sha256:3428d0a90b1e7efbe2ba947bd1d0c9a70594986b4b6553804547f1fb24cfc92f
ENV RUSTUP_TOOLCHAIN=nightly-2026-08-01
RUN rustup toolchain install $RUSTUP_TOOLCHAIN --profile minimal --component rust-src
COPY . $SRC/wallet
WORKDIR $SRC/wallet
COPY .clusterfuzzlite/build.sh $SRC/
