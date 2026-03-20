# Proxy Fee Payer Test (Issue #1381)

E2E test verifying that EVM gas fees are charged to the controller (not the pureProxy) when calling `proxy.proxy(real=pureProxy, call=EVM::call(...))`.

## Problem

When a controller uses `proxy.proxy(real=pureProxy, call=evm.call(...))`, `pallet_proxy` swaps the origin to the pureProxy before EVM execution. By the time `withdraw_fee` runs, the controller's identity is lost and the system tries to charge the pureProxy (which has zero WETH). The `SetEvmFeePayer` transaction extension fixes this by capturing the controller's AccountId before the proxy dispatch and threading it to the fee charging layer.

## Prerequisites

1. Build the node with the fee payer override changes: `cargo build --release`
2. Build a patched chainspec with WETH + testnet mode:
   ```bash
   cd scripts/gas-estimation-test
   npm install
   ./scripts/build-chainspec.sh
   cp chainspec-raw.json ../../
   ```
3. Launch zombienet from the repo root:
   ```bash
   zombienet spawn launch-configs/zombienet/local.json
   ```
   The `local.json` must use `"chain_spec_path": "chainspec-raw.json"` instead of `"chain": "local"` for the parachain.

4. (Optional) Run chain setup if WETH isn't pre-configured in the chainspec:
   ```bash
   cd scripts/proxy-fee-test
   npm install
   WS_URL=ws://127.0.0.1:9999 npm run setup
   ```

## Usage

```bash
cd scripts/proxy-fee-test
npm install
```

### Against local zombienet (default)

```bash
npm test
```

### Against lark testnet

```bash
WS_URL=wss://2.lark.hydration.cloud EVM_RPC_URL=https://2.lark.hydration.cloud npm test
```

### Deploy runtime to lark

```bash
make build-release
npm run deploy:lark
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `WS_URL` | `ws://127.0.0.1:9999` | Substrate WebSocket endpoint |
| `EVM_RPC_URL` | `http://127.0.0.1:9999` | EVM JSON-RPC endpoint |

## Tests

| # | Test | What it verifies |
|---|------|------------------|
| 1 | Direct EVM call | Baseline: gas fees charged to EVM source account via ethers.js (no proxy, no override) |
| 2 | proxy(pureProxy, EVM::call) | Alice creates a pureProxy, calls `proxy.proxy(pureProxy, evm.call(...))`. Gas fees charged to Alice (controller), not the pureProxy |
| 3 | proxy(EVM::call) no funds | Dave (no WETH/HDX) creates a pureProxy and tries the same flow. The inner EVM call fails because the controller can't pay gas |
| 4 | Non-EVM proxy call | Alice proxies a `system.remark` through Bob. Only standard substrate fees apply, no EVM fee override triggered |
| 5 | proxy(batch([EVM::call])) | Alice proxies a `utility.batch([evm.call(...)])` through a pureProxy. The recursive call detection finds the EVM call inside the batch and charges Alice |

## How it works

The test script connects to the node via both Substrate API (`@polkadot/api`) and EVM JSON-RPC (`ethers`).

For proxy tests (2, 3, 5):
1. Creates a pureProxy via `proxy.createPure("Any", 0, 0)` — Alice is automatically the controller
2. Derives the pureProxy's H160 EVM address from its AccountId32 (first 20 bytes)
3. Submits `proxy.proxy(pureProxy, null, evm.call(source=pureProxyEvmAddr, ...))` signed by Alice
4. The `SetEvmFeePayer` extension detects the proxy+EVM pattern and stores Alice's AccountId
5. Verifies Alice's balance decreased (gas charged to controller)

## Related

- Issue: https://github.com/galacticcouncil/HydraDX-node/issues/1381
- Rust unit tests: `cargo test -p hydradx-runtime evm_fee::tests` (12 tests)
- Rust integration tests: `cargo test -p runtime-integration-tests proxy_fee_payer` (6 tests)
