# Init Testnet

## Overview

Simple script to initialize testnet's data. This script initializes omnipool, initializes staking and register assets in the asset-registry.
This script creates a preimage with a batch call. Democracy steps need to be done manually.

## How to

* `npm install`
* `node index.js wss://testnet-rpc` - RPC param is optional. Default RPC is `ws://localhost:9946`

### Democracy steps:
* `Governance -> Preimages` - copy preimage's hash
* `Governance -> Council -> Motions` -  `Propose External` witch copied preimage's hash
* `Governance -> Democracy`- `Fast Track` referenda 
* `Governance -> Tech. comm. -> Proposals` - `Vote` with tech. comm. users and `Close`
* `Governance -> Democracy` - `Vote` for referenda and wait until it's processed and dispatched
