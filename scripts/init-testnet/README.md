# Init Testnet

## Overview

Simple script to initialize testnet's data. This script initializes omnipool, initializes staking and register assets in the asset-registry.
This script creates a preimage with a batch call. Democracy steps need to be done manually.

To register asset with asset's location add `location` to `asset.json` e.g:
```
  "2": {
    "asset": {
      "name": "DAI Stablecoin (via Wormhole)",
      "assetType": "Token",
      "existentialDeposit": "10,000,000,000,000,000",
      "xcmRateLimit": null
    },
    "metadata": {
      "symbol": "DAI",
      "decimals": "18"
    },
    "location": {
      "parents": 1,
      "interior": {
        "X2": [
          {
            "Parachain": 3000
          },
          {
            "GeneralKey": {
              "length": 21,
              "data": "0x0254a37a01cd75b616d63e0ab665bffdb0143c52ae0000000000000000000000"
            }
          }
        ]
      }
    }
  },
```

## How to

* `npm install`
* `node index.js wss://testnet-rpc` - RPC param is optional. Default RPC is `ws://localhost:9946`

### Democracy steps:
* `Governance -> Preimages` - copy preimage's hash
* `Governance -> Council -> Motions` -  `Propose External` witch copied preimage's hash
* `Governance -> Democracy`- `Fast Track` referenda 
* `Governance -> Tech. comm. -> Proposals` - `Vote` with tech. comm. users and `Close`
* `Governance -> Democracy` - `Vote` for referenda and wait until it's processed and dispatched

