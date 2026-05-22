// Chopsticks-specific helpers — shortcut governance and force block production.
//
// Chopsticks doesn't run validators; blocks are only produced on demand via
// the `dev_newBlock` RPC. This module wraps that and provides storage-direct
// shortcuts for governance-gated state (e.g. ApprovedContract registration).

import type { ApiPromise } from "@polkadot/api";
import { xxhashAsHex } from "@polkadot/util-crypto";
import { GIGAHDX_POOL } from "./constants";

export const isChopsticks = (): boolean => !!process.env.CHOPSTICKS;

/**
 * Advance one block via chopsticks' `dev_newBlock` RPC. No-op when not running
 * against chopsticks.
 */
export async function newBlock(api: ApiPromise): Promise<void> {
	if (!isChopsticks()) return;
	const provider = (api as any)._rpcCore.provider;
	await provider.send("dev_newBlock", [{ count: 1 }]);
}

/**
 * Set arbitrary storage via chopsticks' `dev_setStorage`. Pairs is an array
 * of `[key, value]` hex tuples. Value `null` deletes the key.
 */
export async function setStorage(
	api: ApiPromise,
	pairs: Array<[string, string | null]>
): Promise<void> {
	if (!isChopsticks()) throw new Error("setStorage only works on chopsticks");
	const provider = (api as any)._rpcCore.provider;
	await provider.send("dev_setStorage", [pairs]);
}

/**
 * Write `EVMAccounts::ApprovedContract(GIGAHDX_POOL) = ()` directly into
 * storage, bypassing the WhitelistedCaller referendum. Equivalent to the
 * production governance call but instant for tests.
 */
export async function approveGigahdxPoolViaStorage(api: ApiPromise): Promise<void> {
	const palletPrefix = xxhashAsHex("EVMAccounts", 128).replace(/^0x/, "");
	const itemPrefix = xxhashAsHex("ApprovedContract", 128).replace(/^0x/, "");
	const evm = GIGAHDX_POOL.toLowerCase().replace(/^0x/, "");
	// Storage map with `Blake2_128Concat` for EvmAddress key:
	//   key = pallet_prefix(16) || item_prefix(16) || blake2_128(evm)(16) || evm(20)
	// Use the bytesToBlake2_128 helper from polkadot-util-crypto.
	const { blake2AsHex } = await import("@polkadot/util-crypto");
	const evmBytes = Buffer.from(evm, "hex");
	const hashed = blake2AsHex(evmBytes, 128).replace(/^0x/, "");
	const fullKey = "0x" + palletPrefix + itemPrefix + hashed + evm;

	await setStorage(api, [[fullKey, "0x"]]);
	// Don't verify via api.query — chopsticks doesn't push head subscriptions
	// for Manual-mode blocks, so polkadot.js still queries the pre-setStorage
	// head and would hang or return stale data. Trust that setStorage succeeded
	// (it returns synchronously and chopsticks logs confirm the storage write).
}
