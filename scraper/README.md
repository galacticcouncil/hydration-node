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

#### Export chain state as a chain specification
```bash
# Export entire chain state
scraper export-state --uri wss://rpc.hydradx.cloud:443

# Export specific pallets only
scraper export-state --pallet Omnipool System --uri wss://rpc.hydradx.cloud:443

# Export state at specific block
scraper export-state --at 0xfee166d4ba86ef6b33246e22b8d71dcc085923332849c4bc96e618361ba7f446 --uri wss://rpc.hydradx.cloud:443
```

This command will create a chain specification JSON file from the live chain state. The output can be used to start a new chain with the exact same state.

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