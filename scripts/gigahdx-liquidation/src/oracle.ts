// Stress the stHDX price down to push the borrower's health factor below 1.
//
// The FixedPriceOracle deployed at deployment time is `Ownable` (owner = the
// public dev deployer key, hard-coded into lark deployments). We can call
// `setPrice(newPrice)` directly via that key. No governance needed for THIS
// fork-test path; on mainnet the price stress would be a real EMA-backed
// oracle so this helper does not apply there.

import { ethers } from "ethers";
import { DEFAULT_STHDX_PRICE, FIXED_PRICE_ORACLE, WS_URL } from "./constants";

// Public dev deployer key — documented in aave-v3-deploy/deployments/lark2/_addresses.md.
const DEPLOYER_PRIVATE = "0xd9b59470b079ffd6a0373c0870dcf7faf8c20f7340b6d05acbeb8a8a8473b131";

function rpcUrl(): string {
	// JSON-RPC HTTP/WS share the same port on Frontier.
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

/**
 * Drop the stHDX price to crash the borrower's HF below 1.
 * Default target: $0.01 (= 1/2.5 of the base $0.025 price).
 */
export async function dropStHdxPrice(targetPrice: bigint = 1_000_000n): Promise<void> {
	await setPrice(targetPrice);
}

/**
 * Restore the default stHDX price ($0.025) — useful in test teardown so
 * subsequent tests start from a clean slate.
 */
export async function restoreStHdxPrice(): Promise<void> {
	await setPrice(DEFAULT_STHDX_PRICE);
}
