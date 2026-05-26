import type { ApiPromise } from "@polkadot/api";
import {
	BORROWER_EVM,
	BORROWER_URI,
	DEFAULT_FEE,
	DEFAULT_GAS,
	GIGAHDX_POOL,
	HOLLAR,
	STHDX_EVM,
} from "./constants";
import type { ChainContext, KeyringPair } from "./api";
import { signAndWait, pad32, uint32 } from "./utils";

const STAKE_HDX = 100_000n * 10n ** 12n;
const FUND_HDX = 250_000n * 10n ** 12n;
const BORROW_HOLLAR = 80n * 10n ** 18n;

const SEL_SET_USE_RESERVE = "5a3b74b9";
const SEL_BORROW = "a415bcad";

export interface BorrowerHandle {
	signer: KeyringPair;
	substrate: string;
	evm: string;
}

export async function setupBorrower(ctx: ChainContext): Promise<BorrowerHandle> {
	const borrower = ctx.keyring.addFromUri(BORROWER_URI);
	const handle: BorrowerHandle = { signer: borrower, substrate: borrower.address, evm: BORROWER_EVM };

	const stake = (await ctx.api.query.gigaHdx.stakes(borrower.address)) as any;
	if (!stake.isEmpty && BigInt(stake.unwrap().hdx.toString()) >= STAKE_HDX) {
		return handle; // already set up
	}

	// 1. Fund from Bob.
	await signAndWait(
		ctx.api,
		ctx.api.tx.balances.transferKeepAlive(borrower.address, FUND_HDX.toString()),
		ctx.bob,
		"fund-borrower"
	);

	// 2. Bind EVM (skip if already bound).
	try {
		await signAndWait(ctx.api, ctx.api.tx.evmAccounts.bindEvmAddress(), borrower, "borrower.bindEvm");
	} catch (e: any) {
		if (!/AlreadyBound/.test(e.message)) throw e;
	}

	// 3. gigaStake — HDX locked in wallet, stHDX minted to pool, GIGAHDX to borrower EVM.
	await signAndWait(
		ctx.api,
		ctx.api.tx.gigaHdx.gigaStake(STAKE_HDX.toString()),
		borrower,
		"borrower.gigaStake"
	);

	// 4. Enable stHDX as collateral.
	const setUseData = "0x" + SEL_SET_USE_RESERVE + pad32(STHDX_EVM) + uint32(1);
	await signAndWait(
		ctx.api,
		ctx.api.tx.evm.call(
			BORROWER_EVM,
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
		borrower,
		"borrower.setUseAsCollateral"
	);

	// 5. Borrow HOLLAR.
	const borrowData =
		"0x" +
		SEL_BORROW +
		pad32(HOLLAR) +
		uint32(BORROW_HOLLAR) +
		uint32(2) + // variable rate
		uint32(0) + // referralCode
		pad32(BORROWER_EVM);
	await signAndWait(
		ctx.api,
		ctx.api.tx.evm.call(
			BORROWER_EVM,
			GIGAHDX_POOL,
			borrowData,
			0,
			DEFAULT_GAS.toString(),
			DEFAULT_FEE.toString(),
			null,
			null,
			[],
			null
		),
		borrower,
		"borrower.borrow"
	);

	return handle;
}
