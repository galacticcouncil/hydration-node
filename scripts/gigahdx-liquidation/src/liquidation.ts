// Trigger and inspect `pallet-liquidation::liquidate` for a GIGAHDX position.

import type { ApiPromise } from "@polkadot/api";
import type { KeyringPair } from "./api";
import { HOLLAR_ASSET_ID } from "./constants";
import { signAndWait } from "./utils";

export interface LiquidationResult {
	events: any[];
	gigaHdxLiquidated: any | null;
}

/**
 * Submit `pallet_liquidation::liquidate(collateralAssetId, HOLLAR, borrower, amount, [])`
 * and return the block events plus the `GigaHdxLiquidated` event (if any).
 *
 * The OR-clause routing fix in `pallets/liquidation/src/lib.rs` makes BOTH
 * `collateralAssetId == 67` (GIGAHDX aToken) and `670` (stHDX underlying)
 * dispatch to `liquidate_gigahdx`. PEPL uses 670 in production.
 */
export async function liquidate(
	api: ApiPromise,
	signer: KeyringPair,
	collateralAssetId: number,
	borrowerEvm: string,
	amount: bigint,
	debtAssetId: number = HOLLAR_ASSET_ID
): Promise<LiquidationResult> {
	const tx = api.tx.liquidation.liquidate(
		collateralAssetId,
		debtAssetId,
		borrowerEvm,
		amount.toString(),
		[]
	);
	const events = await signAndWait(api, tx, signer, `liquidate(collateral=${collateralAssetId}, amount=${amount})`);

	const gigaHdxLiquidated = events.find(
		({ event }) => event.section === "liquidation" && event.method === "GigaHdxLiquidated"
	)?.event ?? null;

	return { events, gigaHdxLiquidated };
}
