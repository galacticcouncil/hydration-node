// Bob's EVM address is the implicit truncated AccountId32, NOT key-derived (don't call bind)
import type { ApiPromise } from "@polkadot/api";
import { ethers } from "ethers";
import { DEFAULT_FEE, DEFAULT_GAS, WS_URL } from "./constants";
import type { KeyringPair } from "./api";
import { signAndWait, pad32, uint32 } from "./utils";

const MAIN_POOL = "0x1b02E051683b5cfaC5929C25E84adb26ECf87B38";
const WETH_EVM = "0x000000000000000000000000000000010000028e"; // asset 20 multicurrency precompile
const BOB_EVM = "0x8eaf04151687736326c9fea17e25fc5287613693";
const MIN_COLLATERAL_BASE = 1_000_00000000n; // $1k in AAVE base units (1e8)

const SEL_SUPPLY = "617ba037"; // supply(asset, amount, onBehalfOf, referralCode)
const SEL_GET_USER_ACCOUNT_DATA = "bf92857c"; // getUserAccountData(user)
const SEL_APPROVE = "095ea7b3";

function httpRpcUrl(): string {
	return WS_URL.replace(/^ws/, "http");
}

async function getMainCollateralBase(): Promise<bigint> {
	const provider = new ethers.JsonRpcProvider(httpRpcUrl());
	const data = "0x" + SEL_GET_USER_ACCOUNT_DATA + pad32(BOB_EVM);
	const result = await provider.call({ to: MAIN_POOL, data });
	if (!result || result === "0x") return 0n;
	// returns (totalCollateralBase, totalDebtBase, availableBorrows, ...) — slot 0 = collateral
	return BigInt("0x" + result.slice(2, 2 + 64));
}

export async function ensureLiquidator(api: ApiPromise, bob: KeyringPair): Promise<void> {
	const collateral = await getMainCollateralBase();
	if (collateral >= MIN_COLLATERAL_BASE) return;

	const supplyAmount = 10n * 10n ** 18n;

	const approveData = "0x" + SEL_APPROVE + pad32(MAIN_POOL) + uint32(supplyAmount);
	await signAndWait(
		api,
		api.tx.evm.call(
			BOB_EVM,
			WETH_EVM,
			approveData,
			0,
			DEFAULT_GAS.toString(),
			DEFAULT_FEE.toString(),
			null,
			null,
			[],
			null
		),
		bob,
		"bob.approve(WETH→MAIN)"
	);

	const supplyData =
		"0x" +
		SEL_SUPPLY +
		pad32(WETH_EVM) +
		uint32(supplyAmount) +
		pad32(BOB_EVM) +
		uint32(0);
	await signAndWait(
		api,
		api.tx.evm.call(
			BOB_EVM,
			MAIN_POOL,
			supplyData,
			0,
			DEFAULT_GAS.toString(),
			DEFAULT_FEE.toString(),
			null,
			null,
			[],
			null
		),
		bob,
		"bob.supply(MAIN)"
	);
}
