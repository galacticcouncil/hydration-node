# Hydration node

CROSS-CHAIN LIQUIDITY PROTOCOL BUILT ON SUBSTRATE (Formerly HydraDX)

## Contributions & Code of Conduct

Please follow the contributions guidelines as outlined in [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md).
We are welcoming and friendly community please follow our [Code of Conduct](docs/CODE_OF_CONDUCT.md).

## Local Development

Follow these steps to prepare a local Substrate development environment :hammer_and_wrench:

### Simple Setup

Install all the required dependencies with a single command (be patient, this can take up to 30
minutes).

```bash
curl https://getsubstrate.io -sSf | bash -s -- --fast
```

### Manual Setup

Find manual setup instructions at the
[Substrate Developer Hub](https://substrate.dev/docs/en/knowledgebase/getting-started/#manual-installation).

### Build

Once the development environment is set up, build the node:

```bash
cargo build --release
```

## Run

### Mainnet fork with Chopsticks

The easiest way to run and interact with Hydration node is to use [Chopsticks](https://github.com/acalanetwork/chopsticks)

```Bash
npx @acala-network/chopsticks@latest --config=hydradx 
```

Now you have a test node running at [`ws://localhost:8000`](https://polkadot.js.org/apps/?rpc=ws%3A%2F%2Flocalhost%3A8000#/explorer)

This is also best option to test XCM interactions, as you can run multiple parachains at the same time:

```bash
npx @acala-network/chopsticks@latest xcm -p hydradx -p polkadot-asset-hub
```

Refer to the `chopsticks --help` for more options.

### Mainnet fork with Zombienet

If you need to test your app with RPC node which supports everything normal node does like extra RPC methods like `eth_*`. 
This will spawn forked mainnet chain with regular blocktime.

```bash
docker run -d -p 8000:9988 galacticcouncil/fork
```

Now you have a test node running at [`ws://localhost:8000`](https://polkadot.js.org/apps/?rpc=ws%3A%2F%2Flocalhost%3A8000#/explorer)

More details [here](launch-configs/fork)

### Local Testnet with Zombienet

Relay chain repository (polkadot) has to be built in `../polkadot`
Grab `zombienet` utility used to start network from [releases](https://github.com/paritytech/zombienet/releases)

Start local testnet with 4 relay chain validators and HydraDX as a parachain with 2 collators.

```
cd ./rococo-local
zombienet spawn config-zombienet.json
```

## Interaction with the node

Go to the polkadot apps at https://polkadot.js.org/apps

Connect to 
- Mainnet: `wss://rpc.hydradx.cloud`
- local node: `ws://localhost:8000` (if you are using chopsticks)

### Testing of storage migrations and runtime upgrades

The `try-runtime` tool can be used to test storage migrations and runtime upgrades against state from a real chain.
Run the following command to test against the state on Hydration.
Don't forget to use a runtime built with `try-runtime` feature.
```bash
try-runtime --runtime ./target/release/wbuild/hydradx-runtime/hydradx_runtime.wasm on-runtime-upgrade --checks all live --uri wss://rpc.hydradx.cloud:443
```
or against HydraDX testnet on Rococo using `--uri wss://paseo-rpc.play.hydration.cloud:443`

## Useful resources:

* https://github.com/galacticcouncil/hydration-security
* https://apidocs.bsx.fi/HydraDX
* https://docs.hydration.net/

Bug bounty: [https://immunefi.com/bounty/hydration/](https://immunefi.com/bounty/hydration/)
Reponsible disclosure: security@intergalactic.limited
