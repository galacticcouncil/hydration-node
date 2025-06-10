# Hydration Runtime Fuzzer
A fuzzer for the Hydration Runtime which is based on [ziggy](https://github.com/srlabs/ziggy/) and [substrate-runtime-fuzzer](https://github.com/srlabs/substrate-runtime-fuzzer/tree/main) - both developed by [SRLabs](https://github.com/srlabs).

Runs under the hood multiple fuzzers in parallel - [honggfuzz](https://github.com/google/honggfuzz) and [AFL++](https://github.com/aflplusplus/aflplusplus).


## The Fuzz
```
// via Docker

// Go to project root
cd ..

// Build images of fuzzers
docker build -t runtime-fuzzer -f runtime-fuzzer/Dockerfile .

// Run image
docker run -it --entrypoint bash runtime-fuzzer

// Check out -h
cargo ziggy fuzz -h

// Spawn 22 parallel fuzz jobs, 22s timeout to "hang"
cargo ziggy fuzz -t 22 -j 22

More live information by running:
tail -f ./output/hydration-runtime-fuzzer/logs/afl.log
tail -f ./output/hydration-runtime-fuzzer/logs/afl_1.log
tail -f ./output/hydration-runtime-fuzzer/logs/honggfuzz.log
```
