import type { ApiPromise } from "@polkadot/api";
import { ethers } from "ethers";
import { GIGAHDX_POOL, STHDX_EVM, DEFAULT_FEE, DEFAULT_GAS, WS_URL } from "./constants";
import type { KeyringPair } from "./api";
import { signAndWait, pad32, uint32 } from "./utils";

const SEL_SET_USE_RESERVE = "5a3b74b9"; // setUserUseReserveAsCollateral(asset, bool)
const BOB_EVM = "0x8eaf04151687736326c9fea17e25fc5287613693";
const MIN_COLLATERAL_BASE = 1_000_00000000n; // $1k in AAVE base units
const SEL_GET_USER_ACCOUNT_DATA = "bf92857c";
const STAKE_AMOUNT = 1_000_000n * 10n ** 12n; // 1M HDX

function httpRpcUrl(): string {
	return WS_URL.replace(/^ws/, "http");
}

async function getCollateralBase(pool: string, evm: string): Promise<bigint> {
	const provider = new ethers.JsonRpcProvider(httpRpcUrl());
	const data = "0x" + SEL_GET_USER_ACCOUNT_DATA + pad32(evm);
	const result = await provider.call({ to: pool, data });
	if (!result || result === "0x") return 0n;
	return BigInt("0x" + result.slice(2, 2 + 64));
}

// Mirrors Martin's e2e_provision_liq_account: gigaStake → stHDX collateral on GIGAHDX pool
export async function ensureLiquidator(api: ApiPromise, bob: KeyringPair): Promise<void> {
	const collateral = await getCollateralBase(GIGAHDX_POOL, BOB_EVM);
	if (collateral >= MIN_COLLATERAL_BASE) return;

	// Bind EVM (needed so AAVE's aToken credit maps back to Bob's substrate account)
	try {
		await signAndWait(api, api.tx.evmAccounts.bindEvmAddress(), bob, "bob.bindEvm");
	} catch (e: any) {
		if (!/AlreadyBound/.test(e.message)) throw e;
	}

	// gigaStake: locks HDX in wallet, mints stHDX to GIGAHDX pool, mints aToken to Bob
	await signAndWait(
		api,
		api.tx.gigaHdx.gigaStake(STAKE_AMOUNT.toString()),
		bob,
		"bob.gigaStake"
	);

	// Enable stHDX as collateral on GIGAHDX pool so Bob can borrow HOLLAR against it
	const bobEvm = (await api.query.evmAccounts.evmAddresses(bob.address) as any).unwrap().toHex();
	const setUseData = "0x" + SEL_SET_USE_RESERVE + pad32(STHDX_EVM) + uint32(1);
	await signAndWait(
		api,
		api.tx.evm.call(
			bobEvm,
			GIGAHDX_POOL,
			setUseData,
			0,
			DEFAULT_GAS.toString(),
			DEFAULT_FEE.toString(),
			null,
			null,
			[],
			null
		),
		bob,
		"bob.setUseAsCollateral(stHDX)"
	);
}
