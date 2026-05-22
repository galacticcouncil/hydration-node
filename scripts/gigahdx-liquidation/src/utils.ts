import type { ApiPromise } from "@polkadot/api";
import type { SubmittableExtrinsic } from "@polkadot/api/types";
import type { ISubmittableResult } from "@polkadot/types/types";
import type { AddressOrPair } from "@polkadot/api-base/types";
import { isChopsticks } from "./chopsticks";

export const pad32 = (hex: string): string =>
	hex.toLowerCase().replace(/^0x/, "").padStart(64, "0");

export const uint32 = (n: bigint | number): string =>
	pad32(BigInt(n).toString(16));

/**
 * Sign and wait for inclusion. Resolves with all events in the block.
 * Rejects on dispatch error, ExtrinsicFailed, or any evm.ExecutedFailed.
 *
 * Uses `nonce: -1` so polkadot-js queries `accountNextIndex` and picks the
 * next-available nonce — important on a long-running test chain where stale
 * txs may be sitting in the pool and would otherwise cause "Priority is too
 * low" rejections.
 *
 * On chopsticks (Manual block mode), forces `dev_newBlock` after the tx is
 * submitted so it actually lands.
 */
export async function signAndWait(
	api: ApiPromise,
	tx: SubmittableExtrinsic<"promise">,
	signer: AddressOrPair,
	label: string
): Promise<any[]> {
	const onChopsticks = isChopsticks();
	const promise = new Promise<any[]>((resolve, reject) => {
		tx.signAndSend(signer, { nonce: -1 }, (result: ISubmittableResult) => {
			const { status, dispatchError, events } = result;
			if (dispatchError) {
				if (dispatchError.isModule) {
					const d = api.registry.findMetaError(dispatchError.asModule);
					return reject(new Error(`[${label}] ${d.section}.${d.name}: ${d.docs.join(" ")}`));
				}
				return reject(new Error(`[${label}] ${dispatchError.toString()}`));
			}
			if (status.isInBlock) {
				for (const { event } of events) {
					if (event.section === "system" && event.method === "ExtrinsicFailed") {
						return reject(new Error(`[${label}] ExtrinsicFailed: ${event.data.toString()}`));
					}
					if (event.section === "evm" && event.method === "ExecutedFailed") {
						return reject(new Error(`[${label}] evm.ExecutedFailed: ${JSON.stringify(event.toHuman())}`));
					}
				}
				resolve(events as any[]);
			}
		}).catch(reject);
	});

	if (onChopsticks) {
		// Give the tx a moment to enter the pool, then force a new block.
		await new Promise((r) => setTimeout(r, 200));
		try {
			const provider = (api as any)._rpcCore.provider;
			await provider.send("dev_newBlock", [{ count: 1 }]);
		} catch {
			// If newBlock fails (e.g. tx already produced a block), ignore.
		}
	}

	return promise;
}
