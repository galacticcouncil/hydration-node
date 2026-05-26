import type { ApiPromise } from "@polkadot/api";
import { xxhashAsHex } from "@polkadot/util-crypto";
import { GIGAHDX_POOL } from "./constants";

export const isChopsticks = (): boolean => !!process.env.CHOPSTICKS;

export async function newBlock(api: ApiPromise): Promise<void> {
	if (!isChopsticks()) return;
	const provider = (api as any)._rpcCore.provider;
	await provider.send("dev_newBlock", [{ count: 1 }]);
}

export async function setStorage(
	api: ApiPromise,
	pairs: Array<[string, string | null]>
): Promise<void> {
	if (!isChopsticks()) throw new Error("setStorage only works on chopsticks");
	const provider = (api as any)._rpcCore.provider;
	await provider.send("dev_setStorage", [pairs]);
}

// Bypass governance: write ApprovedContract directly to storage
export async function approveGigahdxPoolViaStorage(api: ApiPromise): Promise<void> {
	const palletPrefix = xxhashAsHex("EVMAccounts", 128).replace(/^0x/, "");
	const itemPrefix = xxhashAsHex("ApprovedContract", 128).replace(/^0x/, "");
	const evm = GIGAHDX_POOL.toLowerCase().replace(/^0x/, "");
	// Storage map with `Blake2_128Concat` for EvmAddress key:
	const { blake2AsHex } = await import("@polkadot/util-crypto");
	const evmBytes = Buffer.from(evm, "hex");
	const hashed = blake2AsHex(evmBytes, 128).replace(/^0x/, "");
	const fullKey = "0x" + palletPrefix + itemPrefix + hashed + evm;

	await setStorage(api, [[fullKey, "0x"]]);
	// Skip verify: chopsticks Manual-mode doesn't sync api.query with pre-setStorage head
}
