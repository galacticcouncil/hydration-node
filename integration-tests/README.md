# integration-tests

Full-runtime tests. See also: snapshot-based tests for reproducing and debugging production issues.

## Debugging prod issues with snapshot tests

When something fails on prod (snakewatch alert, user report, error in logs), the fastest reliable loop is: snapshot the chain state from just before the failing block → load it in a Rust test → reproduce → add logs or breakpoints → iterate.

### 1. Identify the failing block

From the prod log, snakewatch alert, or user report, find:
- The block number where the failure happened.
- The block hash *just before* it (so the failing extrinsic/hook hasn't run yet in the snapshot).

### 2. Build the scraper

```bash
cargo build --release -p scraper
```

See `scraper/README.md` for full scraper options.

### 3. Dump chain state into a snapshot

```bash
./target/release/scraper save-storage \
    --uri wss://rpc.hydradx.cloud \
    --at <BLOCK_HASH> \
    --path integration-tests/<pallet>-snapshot
```

Optionally limit which pallets get scraped with `--pallet` to keep the file size manageable. Add `--slim` to drop most user accounts.

This produces `integration-tests/<pallet>-snapshot/SNAPSHOT_<block_number>`.

### 4. Write a test that loads the snapshot

Use `hydra_live_ext` to spin up the runtime against the snapshot. Canonical example: `src/dca.rs:4872`.

```rust
const PATH_TO_SNAPSHOT: &str = "<pallet>-snapshot/SNAPSHOT_<block_number>";

#[ignore] // snapshot may be too big to commit — keep ignored unless you commit the file
#[test]
fn reproduces_prod_bug() {
    TestNet::reset();
    hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
        // call the extrinsic / advance blocks to trigger the failure
    });
}
```

### 5. Iterate — logs or breakpoints

Add `log::info!(target: "...", "...")` calls in the pallet code, then run the test with logs visible:

```bash
export RUST_LOG=<pallet>=debug
cargo test -p runtime-integration-tests <test_name> -- --ignored --nocapture
```

For deeper investigation, set IDE breakpoints (CLion / RustRover / VS Code) and run the test under the debugger.

The log-based loop works just as well for an AI agent: add logs → run the test → read the output → adjust.
