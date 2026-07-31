# meta-tx-relayer

Sponsoring relayer for `pallet-meta-tx`. Accepts a signed intent from a user who holds
nothing, submits it, and pays the fee. The ERC-4337 analogue is a bundler with a
built-in paymaster.

## Why this is not in the node

The relayer holds a funded key and signs transactions. RPC nodes are public and
replicated, so putting a spending key in one means every operator sponsors everyone.
The repo's own precedent agrees: `liquidation-worker-support` exposes an RPC that only
*serves data*, and the actor that signs and submits lives outside the node.

A custom node RPC (`node/src/rpc.rs`, alongside `LiquidationWorker`) is technically
possible and would be the place to put this if sponsorship ever needs to be
consensus-adjacent. It isn't.

## Run

```sh
npm install

RPC_ENDPOINT=ws://localhost:9988 SPONSOR_URI=//Alice npm start   # terminal 1
RPC_ENDPOINT=ws://localhost:9988 npm run demo                    # terminal 2
```

The demo generates a brand-new account with a zero balance, signs a `system.remarkWithEvent`
intent, posts it to the relayer, and prints the account's balance afterwards — still zero.

## API

| Method | Path | Body / result |
|---|---|---|
| `GET` | `/health` | sponsor address and free balance |
| `GET` | `/nonce/:address` | the signer's current meta-tx nonce |
| `POST` | `/sponsor` | `{ signer, call, nonce, deadline, signature }` |

`call` is the SCALE-encoded call as hex. `signature` is a hex `MultiSignature`.

## Configuration

| Variable | Default | Meaning |
|---|---|---|
| `RPC_ENDPOINT` | `ws://localhost:9988` | node websocket |
| `SPONSOR_URI` | `//Alice` | the paying key |
| `PORT` | `3000` | HTTP port |
| `ALLOWED_PALLETS` | `System,Balances,Currencies,Utility,Omnipool` | pallets the sponsor will pay for |
| `MAX_INTENTS_PER_SIGNER` | `20` | per-process quota |
| `MAX_REF_TIME` | `5_000_000_000` | intended weight ceiling |

## What a production deployment still needs

The chain-side primitive is complete; this service is deliberately minimal.

- Persistent rate limiting and quotas (counters here are in-memory and reset on restart).
- Key management — an HSM or remote signer rather than `SPONSOR_URI`.
- Authentication, so sponsorship is not open to the internet.
- Weight screening: `MAX_REF_TIME` is read but not yet enforced against the decoded call.
- Nonce pipelining, so concurrent intents from one signer do not collide.
- Metrics and alerting on sponsor balance.
