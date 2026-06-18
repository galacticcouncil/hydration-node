# signet-feeless-test

Chopsticks e2e for the allowlist-gated, feeless `signet.respond` flow: an
authorized signer holding only the existential deposit (1 HDX) can call
`respond` without paying a fee, and a non-allowlisted account is rejected.

Mirrors the pallet unit tests in `pallets/signet/src/tests/signer_allowlist.rs`.

## Run

Build the runtime wasm and install deps:

```sh
cargo build --release -p hydradx-runtime
cd scripts/signet-feeless-test && yarn install
```

Then run (launches a Chopsticks fork of hydradx, runs the tests, tears down):

```sh
./run.sh
```

`add_signer` is dispatched as Root via the scheduler on the fork; on a live
chain it goes through governance (`UpdateOrigin = EnsureRoot | TechCommittee`).
