# hack HydraDX node

XYK AMM + Exchange order matching blockchain built on substrate for Hackusama hackaton

- https://hack.hydradx.io/
- [app source](https://github.com/galacticcouncil/hack.HydraDX-app)
- [hack submission](https://devpost.com/software/hack-hydra-dx-io)

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

Once the development environment is set up, build the node template. This command will build the
[Wasm](https://substrate.dev/docs/en/knowledgebase/advanced/executor#wasm-execution) and
[native](https://substrate.dev/docs/en/knowledgebase/advanced/executor#native-execution) code:

Build Standalone

```bash
cd node/standalone
cargo build --release
```

Build Parachain
```bash
cargo build --release
```

## Run

### Standalone Node in dev mode
```bash
./node/standalone/target/release/hack-hydra-dx-standalone --dev
```

Purge any existing dev chain state:

```bash
./node/standalone/target/release/hack-hydra-dx-standalone purge-chain --dev
```

Start a dev chain:

```bash
./node/standalone/target/release/hack-hydra-dx-standalone --dev
```

Or, start a dev chain with detailed logging:

```bash
RUST_LOG=debug RUST_BACKTRACE=1 ./node/standalone/target/release/hack-hydra-dx-standalone -lruntime=debug --dev
```

### Interaction with the node

Go to the polkadot apps at https://dotapps.io

Then open settings screen -> developer and paste

*NOTE - FixedU128 type is not yet implemented for polkadot apps. Balance is a measure so price can be reasonably selected. If using polkadot apps to create pool:*
- 1 Mega Units equals 1:1 price
- 20 Mega Units equals 20:1 price
- 50 Kilo Units equals 0.05:1 price

```
{
  "Amount": "i128",
  "AmountOf": "Amount",
  "Address": "AccountId",
  "BalanceInfo": {
    "amount": "Balance",
    "assetId": "AssetId"
  },
  "CurrencyId": "AssetId",
  "CurrencyIdOf": "AssetId",
  "Intention": {
    "who": "AccountId",
    "asset_sell": "AssetId",
    "asset_buy": "AssetId",
    "amount": "Balance",
    "discount": "bool",
    "sell_or_buy": "IntentionType"
  },
  "IntentionId": "u128",
  "IntentionType": {
    "_enum": [
      "SELL",
      "BUY"
    ]
  },
  "LookupSource": "AccountId",
  "Price": "Balance"
}
```

Connect to the `wss://hack.hydradx.io:9944` or local node.

