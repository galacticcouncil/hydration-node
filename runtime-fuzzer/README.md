# HydraDX Runtime Fuzzer
A fuzzer for the HydraDX Runtime which is based on [ziggy](https://github.com/srlabs/ziggy/) and (substrate-runtime-fuzzer)[https://github.com/srlabs/substrate-runtime-fuzzer/tree/main] - both developed by [SRLabs](https://github.com/srlabs).

Runs under the hood multiple fuzzers in parallel - [honggfuzz](https://github.com/google/honggfuzz) and [AFL++](https://github.com/aflplusplus/aflplusplus).

## Installation
```
// Specifically for MacOS
find $HOME/.local -name afl-system-config
sudo #{path_from_output_above}
```

## Running the Fuzzer
```
// Natively (Linux)
cargo install ziggy --version 0.7.0
cargo install cargo-afl honggfuzz grcov
cargo ziggy fuzz -t 20

// via Docker
cd ..
docker build -t runtime-fuzzer -f runtime-fuzzer/Dockerfile .
// Spawn 22 parallel fuzz jobs
cargo ziggy fuzz -j 22
```
