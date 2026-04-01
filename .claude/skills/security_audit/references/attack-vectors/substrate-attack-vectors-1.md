# Attack Vectors — Substrate / Polkadot Common Vulnerabilities

> Sources: Distilled from 30 audit reports across the Polkadot ecosystem (2024–2025) including findings from SRLabs, OAK Security, CoinFabrik, Zellic, Pashov, Beosin, Monethic, and Guvenkaya covering Astar, Acala, Moonbeam, Peaq, InvArch, t3rn, Hyperbridge, Bifrost, KILT, Frontier, Neuroweb, Zeitgeist, LAOS, Virto, Xcavate, and Acurast.

---

## 1. XCM FeeManager Misconfiguration

**Description:** Setting `FeeManager = ()` in XCM executor config waives all XCM execution fees. Attackers send unlimited cross-chain messages at zero cost.

**What to look for:**
- `type FeeManager = ()` in XcmConfig
- Missing fee handler implementation for XCM execution

**Mitigation:** Implement a proper `FeeManager` that charges fees proportional to message weight. Never use `()` for production runtimes.

**Seen in:** InvArch, Hyperbridge, Peaq (4+ runtimes affected)

---

## 2. XCM Delivery Fee Waiver

**Description:** Setting `PriceForSiblingDelivery = ()` removes fees for cross-chain message delivery to sibling parachains, enabling free spam across chains.

**What to look for:**
- `type PriceForSiblingDelivery = ()` in XcmConfig
- Missing pricing for parent/sibling delivery channels

**Mitigation:** Use `ExponentialPrice` or similar delivery fee mechanisms. Ensure all delivery channels charge appropriate fees.

**Seen in:** InvArch, Hyperbridge, Peaq

---

## 3. Overlapping XCM Teleport and Reserve Transfer Trust

**Description:** Configuring the same asset/origin pair as both trusted for teleports (`IsTeleporter`) and reserve transfers (`IsReserve`) creates a drain vector. Attackers reserve-transfer an asset, then teleport it back — the `CheckedAccount` is debited twice for the same asset.

**What to look for:**
- Overlapping `IsTeleporter` and `IsReserve` configurations for the same (asset, location) pair
- Assets trusted for both teleport and reserve from the same origin

**Mitigation:** Ensure teleport and reserve trust are mutually exclusive per asset/origin pair. Audit XCM barrier configs when adding new trusted origins.

**Seen in:** t3rn

---

## 4. XCM Transient State Manipulation (Holding Register)

**Description:** XCM `WithdrawAsset` and `DepositAsset` instructions temporarily burn and re-mint tokens via a holding register. If any pallet uses `total_issuance()` as a pricing oracle, an attacker can manipulate it within a single XCM message execution.

**What to look for:**
- Pallets that read `T::Currency::total_issuance()` for pricing/exchange rate calculations
- `SafeCallFilter = Everything` allowing XCM to call any pallet
- Minting/redemption logic callable via XCM `Transact`

**Mitigation:** Never use `total_issuance()` as a price oracle. Restrict `SafeCallFilter` to a whitelist. Use time-weighted or external price feeds.

**Seen in:** Acala (Homa module — Critical, ~$9.4M at risk)

---

## 5. Zero/Placeholder Weight Configuration

**Description:** Pallets configured with `WeightInfo = ()` (zero weight) or hardcoded test weights instead of benchmarked weights. Extrinsics execute for free or near-free, enabling unlimited spam.

**What to look for:**
- `type WeightInfo = ()` in pallet config
- `Weight::from_parts(100_000, 0)` or similar hardcoded values
- `DbWeight::reads_writes(N, M)` as static weight on variable-complexity extrinsics

**Mitigation:** Benchmark every pallet with `frame_benchmarking`. Never ship `WeightInfo = ()` or hardcoded weights to production.

**Seen in:** InvArch, Hyperbridge, Peaq, t3rn, Acurast

---

## 6. Default/Template Benchmark Weights

**Description:** Using `SubstrateWeight<Runtime>` from the pallet's own default implementation instead of weights benchmarked against the actual runtime. Default weights are generated against a reference machine and may not reflect the real computational cost on the target runtime.

**What to look for:**
- `type WeightInfo = pallet_foo::weights::SubstrateWeight<Runtime>` (default) instead of `weights::pallet_foo::WeightInfo<Runtime>` (benchmarked)
- Weights generated against `substrate-node-template` used in a parachain runtime
- Dependency pallets (balances, assets, utility) using default weights

**Mitigation:** Run benchmarks against the actual runtime on representative hardware. Use runtime-specific weight modules.

**Seen in:** Moonbeam, Peaq, InvArch, Neuroweb

---

## 7. Static Weight on Variable-Complexity Operations

**Description:** Extrinsics with O(n) or O(n*m) complexity annotated with a flat/static weight that doesn't scale with input size. Attackers fill blocks with max-sized inputs at min-sized cost.

**What to look for:**
- Loop-based extrinsics where weight is constant regardless of iteration count
- `on_initialize` / `on_finalize` hooks with static weights that iterate storage
- Weight functions that divide a loop bound instead of multiplying (e.g., `MaxStakers / 100` instead of `MaxStakers`)

**Mitigation:** Parameterize weight functions on input size. Use `#[pallet::weight(T::WeightInfo::foo(x))]` where `x` maps to the variable dimension.

**Seen in:** Bifrost, InvArch (100x underweight), Acurast, Virto, Peaq

---

## 8. Weight Function Reuse Across Different Extrinsics

**Description:** Different extrinsics sharing the same weight function despite having different computational profiles. One may be drastically underweighted.

**What to look for:**
- Multiple `#[pallet::call]` functions referencing the same `WeightInfo` method
- Copy-pasted weight annotations across functions with different DB access patterns

**Mitigation:** Benchmark each extrinsic independently. One weight function per extrinsic.

**Seen in:** t3rn

---

## 9. Missing Inherent Extrinsic Validation

**Description:** Inherent extrinsics (block-author submitted, not user-signed) without `check_inherent` validation allow block producers to set arbitrary values that other validators cannot challenge.

**What to look for:**
- `ProvideInherent` implementations without corresponding `check_inherent`
- Block-author-controlled parameters (gas price targets, timestamps, randomness seeds)

**Mitigation:** Implement `check_inherent` for all inherent extrinsics. Validate against acceptable ranges or consensus rules.

**Seen in:** Frontier (gas price target)

---

## 10. Proxy Type Filter Not Updated on Runtime Upgrades

**Description:** When new pallets with transfer/asset-manipulation capabilities are added to the runtime, existing `ProxyType` filters (e.g., `NonTransfer`) may not be updated to include them. Delegates with restricted proxy types can call the new pallets unrestricted.

**What to look for:**
- `ProxyType::NonTransfer` or similar restrictive filters
- New pallets added to runtime that aren't included in the proxy filter match arms
- `impl InstanceFilter<RuntimeCall>` blocks that use exhaustive matching vs. wildcard

**Mitigation:** Use exhaustive match patterns in proxy filters. Add CI checks that fail when new pallets are added without updating proxy type filters.

**Seen in:** Neuroweb

---

## 11. Insufficient ExistentialDeposit

**Description:** ExistentialDeposit (ED) set too low relative to token price allows cheap mass account creation, permanently bloating chain storage.

**What to look for:**
- ED set to 0 (disables account reaping entirely)
- ED in smallest denomination worth < $0.01 at current token price
- ED not reviewed when token price changes significantly

**Mitigation:** Set ED to a value that makes account creation meaningfully expensive ($0.01–$1.00 range). Review ED when token economics change.

**Seen in:** Peaq (ED=0), Zeitgeist (ED ~$0.00005), Phala (ED=1 out of 10^12)

---

## 12. Missing Storage Deposits on State-Writing Extrinsics

**Description:** Extrinsics that write data to storage without requiring proportional deposits allow cost-free state bloat.

**What to look for:**
- Storage writes in extrinsics without `T::Currency::reserve()` or deposit mechanism
- Approval/allowance creation without `ApprovalDeposit`
- Multisig proposal storage without deposit (e.g., 60KB per proposal)
- Unbounded `Vec<u8>` or `BoundedVec` stored without deposit

**Mitigation:** Charge storage deposits proportional to data size. Use `StorageDepositPerItem` / `StorageDepositPerByte`.

**Seen in:** InvArch (multisig), Peaq (DID/RBAC), Neuroweb (approvals with zero deposit)

---

## 13. Permissive EVM Storage Pricing

**Description:** `GasLimitStorageGrowthRatio` set too low (e.g., 1:1) makes EVM storage allocation extremely cheap relative to gas cost, enabling storage bloat via smart contracts.

**What to look for:**
- `GasLimitStorageGrowthRatio` < 20
- EVM storage costs not aligned with native storage deposit economics

**Mitigation:** Set ratio to make EVM storage costs comparable to native pallet storage deposit costs. Frontier default is 20.

**Seen in:** Peaq

---

## 14. Contract-Under-Construction Bypassing Precompile Restrictions

**Description:** During contract deployment, the contract's `code_len` is zero. If `CallableByContract` checks rely on code length to classify accounts as contracts vs. EOAs, constructors can call precompiles that should be restricted to EOAs only.

**What to look for:**
- `get_address_type()` or similar functions that check `code_len == 0` to identify EOAs
- Precompiles with `CallableByContract` = false
- Constructor-time calls to restricted precompiles

**Mitigation:** Check `is_contract_being_deployed()` in addition to `code_len`. Use a more robust account type classification.

**Seen in:** Frontier

---

## 15. EVM Precompile Gas Mispricing

**Description:** Custom precompiles (cryptographic operations, storage-heavy operations) using placeholder or incorrectly benchmarked gas costs. Underpriced precompiles enable cheap resource exhaustion.

**What to look for:**
- Precompiles sharing gas constants from unrelated operations (e.g., Curve25519 ops using SHA3 costs)
- Custom precompiles without gas benchmarks against actual computation time
- 10x+ discrepancy between gas cost and wall-clock time vs. other operations

**Mitigation:** Benchmark all precompiles independently. Gas costs should reflect actual computation time relative to other EVM operations.

**Seen in:** Frontier (Curve25519 precompiles 10-18x underpriced)

---

## 16. Missing Circuit Breaker / Pause Mechanism

**Description:** Critical operations (cross-chain token wrapping, bridging, minting) without pause functionality. If an upstream dependency is compromised, there's no way to halt the damage.

**What to look for:**
- Token wrapping/minting pallets without `TransactionPause` integration
- Bridge receivers without emergency halt capability
- Operations depending on external trust assumptions (oracle feeds, bridged tokens) without circuit breakers

**Mitigation:** Integrate `pallet-transaction-pause` or equivalent. Ensure governance/technical committee can halt critical paths.

**Seen in:** Neuroweb (TRAC wrapper)

---

## 17. Unbounded Runtime Migration

**Description:** Storage migrations in `on_runtime_upgrade` that iterate over all entries in a storage map without batching. If the map has grown large, the migration exceeds block weight limits.

**What to look for:**
- `on_runtime_upgrade()` with unbounded `for` loops over storage iterators
- Migrations that `translate()` or `drain()` entire storage maps
- No multi-block migration strategy for large datasets

**Mitigation:** Use multi-block migrations (`pallet-migrations`). Set upper bounds on per-block work. Test migrations against production state sizes.

**Seen in:** Hydration (EVM address migration), general pattern across ecosystem

---

## 18. Publicly Callable Cleanup Functions

**Description:** Storage cleanup/garbage collection extrinsics callable by anyone without permission checks. Attackers delete non-expired state or replay deleted messages.

**What to look for:**
- `ensure_signed` on cleanup extrinsics without additional ownership/expiry checks
- Cleanup functions that unconditionally remove entries regardless of TTL/expiry
- Missing nonce/seen-set protection after cleanup allows replay

**Mitigation:** Validate expiry before removal. Restrict cleanup to affected parties or governance. Maintain replay protection independent of storage cleanup.

**Seen in:** Acurast

---

## 19. Incomplete Cross-Chain Message Sender Validation

**Description:** Inbound cross-chain messages validated at the chain level but not at the contract/pallet level. Any payload from the correct chain is accepted regardless of which contract emitted it.

**What to look for:**
- Bridge message handlers that check `source_chain` but not `source_contract`
- Inconsistent validation depth across different chain integrations
- Missing domain separation between multiple proxy contracts on the same chain

**Mitigation:** Validate both chain ID and sender contract/pallet against configured counterparts. Apply consistent validation across all bridge integrations.

**Seen in:** Acurast (AlephZero/Vara chains)

---

## 20. Threshold Signature Deduplication Bypass

**Description:** Multi-signature or oracle threshold verification that counts signatures without deduplicating by public key. The same signer submits their key multiple times to meet the threshold alone.

**What to look for:**
- `check_signatures` or similar functions incrementing a counter per valid (signature, pubkey) pair
- No deduplication of public keys before counting
- Threshold checks like `valid >= min_signatures` without uniqueness enforcement

**Mitigation:** Collect signers into a `BTreeSet` or equivalent before counting. Reject duplicate public keys.

**Seen in:** Acurast
