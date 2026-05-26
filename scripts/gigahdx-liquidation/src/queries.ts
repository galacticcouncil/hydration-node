// Read-only helpers for inspecting on-chain state in tests.

import type { ApiPromise } from "@polkadot/api";
import { ethers } from "ethers";
import { WS_URL, GIGAHDX_POOL } from "./constants";

export interface StakeRecord {
	hdx: bigint;
	gigahdx: bigint;
	frozen: bigint;
	unstaking: bigint;
	unstaking_count: number;
}

export async function queryStakes(
	api: ApiPromise,
	substrateAddress: string
): Promise<StakeRecord | null> {
	const raw = (await api.query.gigaHdx.stakes(substrateAddress)) as any;
	if (raw.isEmpty || raw.isNone) return null;
	const s = raw.unwrap();
	return {
		hdx: BigInt(s.hdx.toString()),
		gigahdx: BigInt(s.gigahdx.toString()),
		frozen: BigInt(s.frozen.toString()),
		unstaking: BigInt(s.unstaking.toString()),
		unstaking_count: Number(s.unstakingCount.toString()),
	};
}

export async function queryTotalLocked(api: ApiPromise): Promise<bigint> {
	const raw = (await api.query.gigaHdx.totalLocked()) as any;
	return BigInt(raw.toString());
}

function httpRpcUrl(): string {
	return WS_URL.replace(/^ws/, "http");
}

const SEL_GET_USER_ACCOUNT_DATA = "bf92857c";
const pad32 = (hex: string): string =>
	hex.toLowerCase().replace(/^0x/, "").padStart(64, "0");

export async function queryHealthFactor(borrowerEvm: string): Promise<bigint> {
	const provider = new ethers.JsonRpcProvider(httpRpcUrl());
	const data = "0x" + SEL_GET_USER_ACCOUNT_DATA + pad32(borrowerEvm);
	const result = await provider.call({ to: GIGAHDX_POOL, data });
	if (!result || result === "0x" || result.length < 2 + 6 * 64) return 0n;
	// getUserAccountData returns 6 uint256 slots; healthFactor is slot[5]
	return BigInt("0x" + result.slice(2 + 5 * 64, 2 + 6 * 64));
}

export async function queryOraclePrice(oracleAddress: string): Promise<bigint> {
	const provider = new ethers.JsonRpcProvider(httpRpcUrl());
	const oracle = new ethers.Contract(
		oracleAddress,
		["function latestAnswer() view returns (int256)"],
		provider
	);
	return (await oracle.latestAnswer()) as bigint;
}
