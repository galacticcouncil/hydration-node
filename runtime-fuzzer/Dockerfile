FROM rustlang/rust:nightly

RUN apt-get update
RUN apt-get install -y protobuf-compiler clang binutils-dev libunwind-dev

COPY ./ ./

WORKDIR /runtime-fuzzer

RUN cargo install cargo-afl honggfuzz grcov ziggy

RUN cargo ziggy build
