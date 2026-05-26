import type { ApiPromise } from "@polkadot/api";
import type { SubmittableExtrinsic } from "@polkadot/api/types";
import { GIGAHDX_POOL } from "./constants";
import type { KeyringPair } from "./api";
import { signAndWait } from "./utils";
import { isChopsticks, approveGigahdxPoolViaStorage } from "./chopsticks";

// Without approval, HOLLAR's delegatedToken path returns 0 → Panic(0x11) in liquidationCall
export async function ensureGigahdxPoolApproved(
	api: ApiPromise,
	alice: KeyringPair
): Promise<void> {
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

// TC.propose(threshold=1) inline-executes for Alice (sole TC member)
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
