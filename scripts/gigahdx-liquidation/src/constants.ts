// Lark2 deployment constants. Mirrors aave-v3-deploy/deployments/lark2/_addresses.json.

export const WS_URL = process.env.WS_URL || "ws://127.0.0.1:9999";

// AAVE V3 GIGAHDX instance — second money market on Hydration.
export const GIGAHDX_POOL = "0xb952AE92cC4D8D703d2d71Ab541baB34c94b944A";
export const GIGAHDX_AAVE_ORACLE = "0x1f14A240f5Aa8eDD4C5f375B82b3B1d836eF4983";
export const GIGAHDX_LOCKABLE_ATOKEN = "0x25fA2B5a75ECDF39BA194fc96AAc12682DB42661";

// Reserve assets.
export const HOLLAR = "0x531a654d1696ED52e7275A8cede955E82620f99a";
export const STHDX_EVM = "0x000000000000000000000000000000010000029e";

// Substrate asset ids (mirror runtime/hydradx/src/assets.rs).
export const STHDX_ASSET_ID = 670;
export const GIGAHDX_ASSET_ID = 67;
export const HOLLAR_ASSET_ID = 222;

// Existing FixedPriceOracle (writable via `setPrice` by the deployer key).
export const FIXED_PRICE_ORACLE = "0x60391660c136046bB7ac5E86E416617df8f5dAa3";

// Default (un-stressed) stHDX price in AAVE base units (1e8 = $1).
export const DEFAULT_STHDX_PRICE = 2_500_000n; // $0.025

// EVM gas defaults for substrate-side evm.call.
export const DEFAULT_GAS = 500_000n;
export const DEFAULT_FEE = 25n * 10n ** 9n; // 25 gwei

// Test borrower account — derived from //LIQTEST_BORROWER.
export const BORROWER_URI = "//LIQTEST_BORROWER";
export const BORROWER_EVM = "0x82ca6a75959daf901249c52abf91de0444f157c1";
