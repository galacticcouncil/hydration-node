// Helpers for invoking governance-gated runtime calls in the test environment.
//
// We submit through TC.propose(threshold=1) because Alice is the sole tech
// committee member on the local zombienet — the proposal executes inline.
// On production lark / mainnet the same calls go through a WhitelistedCaller
// or GeneralAdmin referendum (see aave-v3-deploy/scripts/lark/approve-gigahdx-as-controller.ts).

import type { ApiPromise } from "@polkadot/api";
import type { SubmittableExtrinsic } from "@polkadot/api/types";
import { GIGAHDX_POOL } from "./constants";
import type { KeyringPair } from "./api";
import { signAndWait } from "./utils";
import { isChopsticks, approveGigahdxPoolViaStorage } from "./chopsticks";

/**
 * Approve GIGAHDX pool as an EVM controller via TC majority.
 *
 * Without this, HOLLAR's `delegatedToken` (HDX precompile) returns 0 for
 * `allowance(_, pool)` instead of MAX, HOLLAR falls back to its internal
 * allowance check, that allowance is 0 because pallet-liquidation never
 * approves HOLLAR for the pool, and `allowance - amount` underflows with
 * checked math → `Panic(0x11)` inside `Pool.liquidationCall`.
 *
 * Idempotent — returns early if the pool is already approved.
 */
export async function ensureGigahdxPoolApproved(
	api: ApiPromise,
	alice: KeyringPair
): Promise<void> {
	// Chopsticks shortcut: write storage directly via dev_setStorage. We skip
	// the existence-check too because chopsticks may not surface storage reads
	// against the pre-setStorage head reliably.
	if (isChopsticks()) {
		await approveGigahdxPoolViaStorage(api);
		return;
	}

	const existing = (await api.query.evmAccounts.approvedContract(GIGAHDX_POOL)) as any;
	if (!existing.isEmpty) return;

	const inner = api.tx.evmAccounts.approveContract(GIGAHDX_POOL);
	await proposeViaTc(api, alice, inner, "approveContract(GIGAHDX_POOL)");

	const after = (await api.query.evmAccounts.approvedContract(GIGAHDX_POOL)) as any;
	if (after.isEmpty) {
		throw new Error("approveContract did not take effect — origin filter rejected?");
	}
}

/**
 * Submit a call via `TC.propose(threshold=1)`. Inline-executes for Alice (sole TC member).
 */
export async function proposeViaTc(
	api: ApiPromise,
	alice: KeyringPair,
	inner: SubmittableExtrinsic<"promise">,
	label: string
): Promise<void> {
	const propose = api.tx.technicalCommittee.propose(1, inner, inner.encodedLength);
	const events = await signAndWait(api, propose, alice, `TC.propose(${label})`);

	for (const { event } of events) {
		if (event.section === "technicalCommittee" && event.method === "Executed") {
			const data = event.data.toJSON() as any;
			const result = data?.[1]?.result ?? data?.result;
			if (result && result !== "Ok" && (result.err || result.Err)) {
				throw new Error(`TC.Executed(${label}) inner err: ${JSON.stringify(result)}`);
			}
		}
	}
}
