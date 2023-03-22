# Scraper tool

### Overview

Scraper tool can be used to scrape storage and/or transactions from a live chain and store them in a file.
This file can then be used to write tests against the state stored in the file or apply the stored transactions.

## Build

```bash
cargo build --release -p scraper
```

## Usage examples

#### Store the latest state for `Omnipool` and related pallets
```bash
./target/debug/scraper save-storage --pallet Omnipool System AssetRegistry Balances Tokens --uri wss://rpc.hydradx.cloud:443
```

#### Store the entire state at some block
```bash
./target/debug/scraper save-storage --at 0xfee166d4ba86ef6b33246e22b8d71dcc085923332849c4bc96e618361ba7f446 --uri wss://rpc.hydradx.cloud:443
```

#### Store five consecutive blocks, starting from the block number `2039120`
```bash
scraper --uri wss://rpc.hydradx.cloud:443 save-blocks 2039120 5
```

#### Test

```rust
#[test]
fn test_with_stored_state_and_txs() {
    hydra_live_ext().execute_with(|| { 
        whitelisted_pallets: vec!["Tokens", "Balances"]; // only calls from the Tokens and Balances pallets will be applied
        apply_blocks_from_file(whitelisted_pallets);
    });
}
```