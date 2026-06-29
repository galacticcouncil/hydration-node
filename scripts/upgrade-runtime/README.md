# Runtime upgrade

Deploys a runtime upgrade to a Hydration fork or testnet via governance.

## Flow

1. Submits a referendum on the Root track with `system.authorizeUpgrade(wasmHash)` as an inline call
2. Places the decision deposit
3. Votes aye with a large HDX amount (default 3B)
4. Waits for the referendum to pass and the scheduled call to execute
5. Submits unsigned `system.applyAuthorizedUpgrade(wasm)`
6. Waits for `specVersion` to change

## Usage

```bash
npm install

# Download the runtime WASM
curl -sLO https://github.com/galacticcouncil/hydration-node/releases/latest/download/hydradx_runtime.compact.compressed.wasm

# Run the upgrade
RPC=wss://node5.lark.hydration.cloud \
WASM=./hydradx_runtime.compact.compressed.wasm \
SURI=//Alice \
  npm run upgrade
```

## Environment variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RPC` | `ws://127.0.0.1:9944` | WebSocket endpoint of the target chain |
| `WASM` | `./hydradx_runtime.compact.compressed.wasm` | Path to runtime WASM |
| `SURI` | `//Alice` | Signing key URI |
| `VOTE_HDX` | `3000000000` | Vote amount in whole HDX |
| `ENACT_AFTER` | `10` | Blocks until enactment after approval |

## Requirements

- Target chain with OpenGov (referenda / conviction voting pallets)
- Signing account with enough HDX for decision deposit + vote balance
- On fork networks, `//Alice` is typically pre-endowed

## Notes

- Only works on chains where the Root track is accessible to the signer (e.g. fork environments that endow `//Alice`)
- On mainnet, use the standard governance flow via polkadot.js UI or Subsquare
