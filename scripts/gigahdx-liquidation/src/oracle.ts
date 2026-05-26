// Fork-test only: FixedPriceOracle is Ownable; mainnet uses real EMA oracle
import { ethers } from "ethers";
import { DEFAULT_STHDX_PRICE, FIXED_PRICE_ORACLE, WS_URL } from "./constants";

const DEPLOYER_PRIVATE = "0xd9b59470b079ffd6a0373c0870dcf7faf8c20f7340b6d05acbeb8a8a8473b131";

function rpcUrl(): string {
	return WS_URL.replace(/^ws/, "http");
}

async function setPrice(newPrice: bigint): Promise<void> {
	const provider = new ethers.JsonRpcProvider(rpcUrl());
	const signer = new ethers.Wallet(DEPLOYER_PRIVATE, provider);
	const oracle = new ethers.Contract(
		FIXED_PRICE_ORACLE,
		["function setPrice(int256) external", "function latestAnswer() view returns (int256)"],
		signer
	);
	const tx = await oracle.setPrice(newPrice);
	await tx.wait();

	const after = (await oracle.latestAnswer()) as bigint;
	if (after !== newPrice) {
		throw new Error(`setPrice did not take effect — wanted ${newPrice}, got ${after}`);
	}
}

export async function dropStHdxPrice(targetPrice: bigint = 1_000_000n): Promise<void> {
	await setPrice(targetPrice);
}

export async function restoreStHdxPrice(): Promise<void> {
	await setPrice(DEFAULT_STHDX_PRICE);
}
