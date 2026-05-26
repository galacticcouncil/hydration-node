import type { ApiPromise } from "@polkadot/api";
import type { KeyringPair } from "./api";
import { HOLLAR_ASSET_ID } from "./constants";
import { signAndWait } from "./utils";

export interface LiquidationResult {
	events: any[];
	gigaHdxLiquidated: any | null;
}

// Both collateralAssetId 67 and 670 route to liquidate_gigahdx (PEPL uses 670)
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
