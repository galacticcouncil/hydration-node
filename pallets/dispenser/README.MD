# Dispenser Pallet (`pallet-dispenser`)

Pallet for requesting **Ethereum gas (ETH)** from an external EVM faucet via **SigNet** from within the HydraDX chain.

This pallet:

- Builds a typed EVM transaction calling `IGasFaucet::fund(address to, uint256 amount)`.
- Requests a signature from **SigNet** using `pallet_signet`.
- Charges a fee in a configured **fee asset** (e.g. HDX, `AssetId = 0`).
- Collects a configured **faucet asset** (e.g. WETH, `AssetId = 20`) from the user as collateral.
- Tracks the remote faucet’s **ETH balance in wei** and enforces a minimum on-chain threshold.
- Can be **paused/unpaused** and have its tracked balance adjusted by governance.

---

### Build the pallet only

From the repo root:

```bash
# Build the pallet crate
cargo build -p pallet-dispenser
```

### Run pallet unit tests

```bash
cargo test -p pallet-dispenser
```

---

## 1. Runtime Configuration

The pallet is wired into the runtime via:

```rust
impl pallet_dispenser::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = FungibleCurrencies<Runtime>;
    type MinimumRequestAmount = SigEthFaucetMinRequest;
    type MaxDispenseAmount = SigEthFaucetMaxDispense;
    type DispenserFee = SigEthFaucetDispenserFee;
    type FeeAsset = SigEthFaucetFeeAssetId;
    type FaucetAsset = SigEthFaucetFaucetAssetId;
    type FeeDestination = SigEthFaucetTreasuryAccount;
    type FaucetAddress = SigEthFaucetContractAddr;
    type PalletId = SigEthPalletId;
    type MinFaucetEthThreshold = SigEthMinFaucetThreshold;
    type WeightInfo = weights::pallet_dispenser::HydraWeight<Runtime>;
}
```

### 2.1. Constants

These constants are defined in the runtime (snippet):

```rust
parameter_types! {
    pub const SigEthPalletId: PalletId = PalletId(*b"py/fucet");
    pub const SigEthFaucetDispenserFee: u128 = 5_000;
    pub const SigEthFaucetMaxDispense: u128 = 1_000_000_000_000_000_000;
    pub const SigEthFaucetMinRequest: u64 = 0;
    pub const SigEthFaucetFeeAssetId: AssetId = 0;
    pub const SigEthFaucetFaucetAssetId: AssetId = 20;
    pub const SigEthMinFaucetThreshold: u128 = 50_000_000_000_000_000u128;
}
```

```rust
pub struct SigEthFaucetTreasuryAccount;
impl frame_support::traits::Get<AccountId> for SigEthFaucetTreasuryAccount {
    fn get() -> AccountId {
        Treasury::account_id()
    }
}

pub struct SigEthFaucetContractAddr;
impl frame_support::traits::Get<[u8; 20]> for SigEthFaucetContractAddr {
    fn get() -> [u8; 20] {
        [
            0x52, 0xBE, 0x07, 0x7E, 0x67, 0x49, 0x6C, 0x97, 0x63, 0xCC,
            0xEF, 0x66, 0xC1, 0x11, 0x7D, 0xD2, 0x34, 0xCA, 0x8C, 0xFC,
        ]
    }
}
```

#### Meaning of parameters

- `SigEthPalletId`
  Used to derive the pallet’s sovereign account (`Pallet::<T>::account_id()`), which is the logical sender for SigNet requests.

- `SigEthFaucetDispenserFee` (`DispenserFee`)
  Flat fee charged **per request**, denominated in `FeeAsset`.
  Example: `5_000` units of asset ID `0` (HDX) per request.

- `SigEthFaucetMaxDispense` (`MaxDispenseAmount`)
  **Maximum amount of faucet asset** (`FaucetAsset`) a user can request in a single call.

- `SigEthFaucetMinRequest` (`MinimumRequestAmount`)
  **Minimum request amount**. `0` means no lower bound at the pallet level (caller may still impose their own UX limits).

- `SigEthFaucetFeeAssetId` (`FeeAsset`)
  Asset used to pay the dispensation fee.
  In this config: `0` (HDX).

- `SigEthFaucetFaucetAssetId` (`FaucetAsset`)
  Asset used as collateral / representation of remote ETH on HydraDX.
  In this config: `20` (e.g. WETH).

- `SigEthMinFaucetThreshold` (`MinFaucetEthThreshold`)
  Minimum ETH (in **wei**) that must remain in the external EVM faucet **after** servicing a request.
  If `FaucetBalanceWei - amount < SigEthMinFaucetThreshold`, the request is rejected.

- `SigEthFaucetTreasuryAccount` (`FeeDestination`)
  Account that receives:

  - The fee in `FeeAsset`
  - The requested amount of `FaucetAsset` from the user as collateral

- `SigEthFaucetContractAddr` (`FaucetAddress`)
  The 20‑byte **EVM address** of the deployed **GasFaucet** contract on the target EVM chain.

---

## 3. Wiring the EVM Faucet Address

The `FaucetAddress` is configured via `SigEthFaucetContractAddr::get()`.
To point to a different faucet contract (e.g. testnet vs mainnet), change this constant.

### 3.1. Example: using a different address

```rust
pub struct SigEthFaucetContractAddr;
impl frame_support::traits::Get<[u8; 20]> for SigEthFaucetContractAddr {
    fn get() -> [u8; 20] {
        // Replace with your deployed GasFaucet address
        hex_literal::hex!("1234567890abcdef1234567890abcdef12345678")
    }
}
```

After editing:

1. Rebuild the runtime:
   ```bash
   cargo build -p hydradx-runtime --release
   ```
2. Perform a **runtime upgrade** using governance (as per HydraDX standard process).

> **Important:**
> The EVM address here **must** correspond to a deployed `GasFaucet` contract that exposes the `fund(address,uint256)` function, otherwise the external call will revert.

---

## 4. Relationship With `pallet_signet` and External GasFaucet

This pallet depends on `pallet_signet` and an external EVM GasFaucet contract:

- On‑chain:

  - `pallet_dispenser::request_fund` constructs an EIP‑1559 transaction targeting `FaucetAddress`.
  - It calls `IGasFaucet::fund(to, amount)` encoded via `alloy_sol_types`.
  - It then uses `pallet_signet::sign_bidirectional` to request an ECDSA signature from SigNet.

- Off‑chain:
  - The SigNet MPC network signs and broadcasts the EVM transaction to the configured chain.
  - The external `GasFaucet` contract sends ETH to `to` or issues vouchers (depending on your GasFaucet logic).

The runtime config for `pallet_signet` is already present in the snippet:

```rust
parameter_types! {
    pub const SignetPalletId: PalletId = PalletId(*b"py/signt");
    pub const MaxChainIdLength: u32 = 128;
    pub const MaxEvmDataLength: u32 = 100_000;
    pub const MaxSignatureDeposit: Balance = 200_000_000_000_000;
}

impl pallet_signet::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type PalletId = SignetPalletId;
    type MaxChainIdLength = MaxChainIdLength;
    type WeightInfo = weights::pallet_signet::HydraWeight<Runtime>;
    type MaxDataLength = MaxEvmDataLength;
    type UpdateOrigin = EnsureRoot<AccountId>;
    type MaxSignatureDeposit = MaxSignatureDeposit;
}
```

Make sure:

- `pallet_signet` is properly configured and operational.
- The off‑chain SigNet infrastructure is connected to your chain and the target EVM.

---

## 5. Operating the Dispenser On‑Chain

### 5.1. Initial Setup Checklist

1. **Assets exist** in `pallet_asset_registry`:

   - `FeeAsset` (`SigEthFaucetFeeAssetId`) – e.g. HDX (0)
   - `FaucetAsset` (`SigEthFaucetFaucetAssetId`) – e.g. WETH (20)

2. **Treasury account** has been created and is accessible via `Treasury::account_id()`.

3. **External GasFaucet contract** is deployed on the target EVM chain and funded with ETH.

4. `SigEthFaucetContractAddr` is set to the deployed GasFaucet address and the runtime has been upgraded.

5. `FaucetBalanceWei` is updated to approximate the real ETH balance (see below).

### 5.2. Updating the Tracked Faucet Balance

The pallet maintains an **internal accounting balance** of the external EVM faucet’s ETH in the storage item:

```rust
#[pallet::storage]
pub type FaucetBalanceWei<T> = StorageValue<_, Balance, ValueQuery>;
```

Governance uses the `set_faucet_balance` extrinsic to **add** to this value:

- Call: `pallet_dispenser::set_faucet_balance(balance_wei)`
- Origin: `UpdateOrigin` (in runtime snippet this is `EnsureRoot` via `pallet_signet::Config`)

Example flows:

- **After funding the EVM faucet with +1 ETH** (on the EVM chain):

  - Convert ETH → wei: `1 ETH = 1_000_000_000_000_000_000 wei`.
  - Call `set_faucet_balance(1_000_000_000_000_000_000)` from Root.
  - `FaucetBalanceWei` is increased by that amount.

- **The pallet itself decrements** `FaucetBalanceWei` by `amount` on every successful `request_fund`.

> This is a best‑effort accounting guardrail. Operators should periodically reconcile the on‑chain `FaucetBalanceWei` with the actual EVM faucet balance and correct via `set_faucet_balance` if needed.

### 5.3. Pausing / Unpausing

The pallet supports an emergency pause switch stored in `DispenserConfig`:

- `pause()`

  - Origin: `UpdateOrigin` (Root / governance)
  - Sets `DispenserConfig { paused: true }`.
  - All future `request_fund` calls will fail with `Error::Paused`.

- `unpause()`
  - Origin: `UpdateOrigin`
  - Sets `paused = false`.
  - Requests are allowed again.

Usage:

- During an incident (e.g. suspected faucet compromise, SigNet issues, accounting mismatch), governance can call `pause()`.
- After resolving the issue and adjusting configuration/balance, call `unpause()`.

---

## 6. User‑Facing Flow: `request_fund`

From a user’s perspective:

1. They call `pallet_dispenser::request_fund` with:

   - `to: [u8; 20]` – EVM address to receive ETH
   - `amount: Balance` – requested ETH in wei
   - `request_id: Bytes32` – computed off‑chain using the same `generate_request_id` logic
   - `tx: EvmTransactionParams` – gas limit, gas prices, nonce, EVM chain ID, etc.

2. The pallet:

   - Checks `paused`, amount bounds, faucet threshold.
   - Verifies the `request_id` matches the internally derived ID.
   - Deducts:
     - `DispenserFee` in `FeeAsset` from user → `FeeDestination`
     - `amount` of `FaucetAsset` from user → `FeeDestination`
   - Builds EIP‑1559 transaction bytes and calls `pallet_signet::sign_bidirectional`.
   - Marks `request_id` as used and decrements `FaucetBalanceWei`.

3. SigNet eventually signs and broadcasts the EVM transaction, and the user receives ETH on `to`.

---

## 8. Summary of Runtime Touchpoints

To integrate / modify the dispenser pallet:

1. **Set pallet constants** in `runtime/src/lib.rs`:

   - `SigEthPalletId`
   - `SigEthFaucetDispenserFee`
   - `SigEthFaucetMaxDispense`
   - `SigEthFaucetMinRequest`
   - `SigEthFaucetFeeAssetId`
   - `SigEthFaucetFaucetAssetId`
   - `SigEthMinFaucetThreshold`
   - `SigEthFaucetTreasuryAccount`
   - `SigEthFaucetContractAddr`

2. Wire them into `impl pallet_dispenser::Config for Runtime`.

3. Ensure `pallet_signet::Config` is present and correct.

4. Rebuild & apply a **runtime upgrade**.

5. From governance:
   - Set initial `FaucetBalanceWei` via `set_faucet_balance`.
   - Use `pause`/`unpause` when necessary.

Once these steps are done, users can start calling `request_fund` to obtain ETH on the configured EVM destination chain via the SigNet‑backed GasFaucet.
