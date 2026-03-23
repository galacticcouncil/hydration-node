# Proxy Fee Payer Test (Issue #1381)

E2E test verifying that EVM gas fees are charged to the controller (not the pureProxy) when using `dispatcher.dispatchWithFeePayer(proxy.proxy(real=pureProxy, call=EVM::call(...)))`.

## Problem

When a controller uses `proxy.proxy(real=pureProxy, call=evm.call(...))`, `pallet_proxy` swaps the origin to the pureProxy before EVM execution. The `dispatcher.dispatchWithFeePayer` extrinsic sets the controller as the EVM fee payer before dispatching the inner call, so gas fees are charged to the controller instead of the pureProxy.

## Prerequisites

1. Build the node: `cargo build --release`
2. Launch zombienet or chopsticks
3. Configure WETH if needed:
   ```bash
   cd scripts/proxy-fee-test
   npm install
   WS_URL=ws://127.0.0.1:9999 npm run setup
   ```

## Usage

```bash
cd scripts/proxy-fee-test
npm install

# Against local zombienet (default)
npm test

# Against lark testnet
WS_URL=wss://2.lark.hydration.cloud EVM_RPC_URL=https://2.lark.hydration.cloud npm test
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `WS_URL` | `ws://127.0.0.1:9999` | Substrate WebSocket endpoint |
| `EVM_RPC_URL` | `http://127.0.0.1:9999` | EVM JSON-RPC endpoint |

## Tests

| # | Test | Pattern |
|---|------|---------|
| 1 | Direct EVM call (baseline) | `eth_sendTransaction` |
| 2 | `dispatchWithFeePayer(proxy(EVM::call))` | Simple wrapper |
| 3 | `batchAll([remark, dispatchWithFeePayer(proxy(EVM::call))])` | Batch wrapping |
| 4 | `batchAll([dispatchWithExtraGas, bind, dispatchWithFeePayer(proxy(EVM::call))])` | Full lark UI pattern |
| 5 | `dispatchWithFeePayer` fails with no-funds controller | Error case |
| 6 | Non-EVM proxy call works normally | No interference |

## How it works

```
utility.batchAll([
  dispatcher.dispatchWithExtraGas(currencies.transfer(...), extra_gas),
  proxy.proxy(real, type, evmAccounts.bindEvmAddress),
  dispatcher.dispatchWithFeePayer(
    proxy.proxy(real, type, evm.call(...))
  )
])
```

1. Controller signs the outer `batchAll`
2. `dispatchWithFeePayer` captures the signer as the EVM fee payer
3. Inner `proxy.proxy` swaps origin to pureProxy, dispatches `evm.call`
4. EVM gas fees are charged to the controller via the fee payer override
5. After dispatch, the fee payer is restored (supports nesting)

## Related

- Issue: https://github.com/galacticcouncil/HydraDX-node/issues/1381
- Rust pallet tests: `cargo test -p pallet-dispatcher`
- Rust integration tests: `cargo test -p runtime-integration-tests proxy_fee_payer`
