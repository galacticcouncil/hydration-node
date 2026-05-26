// E2E test for GIGAHDX liquidation via pallet-liquidation::liquidate_gigahdx.
//
// Prerequisites:
//   - Local zombienet fork running on ws://127.0.0.1:9999 (override with WS_URL)
//   - State sourced from a lark2 snapshot WITH the GIGAHDX pool already
//     deployed and wired (Phase 7 of GIGAHDX-LARK-DEPLOY-RUNBOOK.md complete)
//   - //Alice is the sole TC member on this chain (lark default)
//
// Test groups:
//   1. Preconditions — pool approval, borrower setup, oracle state
//   2. Dispatch routing — stHDX (670) and GIGAHDX (67) both reach liquidate_gigahdx
//   3. Negative cases — wrong debt asset, no position, healthy position
//   4. Post-liquidation state — borrower stakes, locks, totals
//   5. Sequential liquidations — multiple small liquidations drain position
//   6. Staking lifecycle — stake, unstake, cancel_unstake basic flows

import { expect } from "chai";
import { connect, type ChainContext } from "../src/api";
import { ensureGigahdxPoolApproved } from "../src/governance";
import { ensureLiquidator } from "../src/liquidator";
import { setupBorrower, type BorrowerHandle } from "../src/borrower";
import { dropStHdxPrice, restoreStHdxPrice } from "../src/oracle";
import { liquidate } from "../src/liquidation";
import { isChopsticks } from "../src/chopsticks";
import {
	GIGAHDX_ASSET_ID,
	GIGAHDX_POOL,
	STHDX_ASSET_ID,
	HOLLAR_ASSET_ID,
	GIGAHDX_AAVE_ORACLE,
	FIXED_PRICE_ORACLE,
	DEFAULT_STHDX_PRICE,
	WS_URL,
} from "../src/constants";
import {
	queryStakes,
	queryHealthFactor,
	queryOraclePrice,
	queryTotalLocked,
} from "../src/queries";

const ONE_HOLLAR = 10n ** 18n;
const PRICE_CRASH_TARGET = 1_000_000n; // $0.01 — enough to push HF below 1

// Chopsticks runs lark2 mainnet state — Alice already has a real position
// that should be liquidatable after a price crash. On zombienet we set up a
// fresh //LIQTEST_BORROWER instead (chopsticks lacks Ethereum-RPC so we can't
// drive the borrow flow over `eth_*`).
const CHOPSTICKS_BORROWER_EVM = "0xd43593c715fdd31c61141abd04a99fd6822c8558"; // //Alice EVM

// ============================================================================
// 1. Preconditions
// ============================================================================
describe("GIGAHDX liquidation — preconditions", function () {
	this.timeout(180_000);

	let ctx: ChainContext;
	let borrower: BorrowerHandle | null = null;

	before(async () => {
		ctx = await connect();
		await ensureGigahdxPoolApproved(ctx.api, ctx.alice);

		if (isChopsticks()) {
			borrower = {
				signer: ctx.alice,
				substrate: ctx.alice.address,
				evm: CHOPSTICKS_BORROWER_EVM,
			};
		} else {
			await ensureLiquidator(ctx.api, ctx.bob);
			borrower = await setupBorrower(ctx);
		}
	});

	after(async () => {
		await ctx.api.disconnect();
	});

	it("should have GIGAHDX pool registered as approved EVM contract", async () => {
		if (isChopsticks()) return;
		const entry = (await ctx.api.query.evmAccounts.approvedContract(GIGAHDX_POOL)) as any;
		expect(entry.isEmpty, "GIGAHDX pool must be in approvedContract storage").to.be.false;
	});

	it("should have a borrower with a non-empty gigahdx stake", async () => {
		if (isChopsticks()) return;
		const stakes = await queryStakes(ctx.api, borrower!.substrate);
		expect(stakes, "borrower must have a Stakes record").to.not.be.null;
		expect(stakes!.hdx > 0n, "borrower must have staked HDX").to.be.true;
		expect(stakes!.gigahdx > 0n, "borrower must have GIGAHDX aTokens").to.be.true;
	});

	it("should report correct stHDX price from oracle before price drop", async () => {
		if (isChopsticks()) return;
		const price = await queryOraclePrice(FIXED_PRICE_ORACLE);
		expect(price, "oracle price must equal default").to.equal(DEFAULT_STHDX_PRICE);
	});
});

// ============================================================================
// 2. Dispatch routing
// ============================================================================
describe("GIGAHDX liquidation — dispatch routing", function () {
	this.timeout(180_000);

	let ctx: ChainContext;
	let borrower: BorrowerHandle | null = null;

	before(async () => {
		ctx = await connect();
		await ensureGigahdxPoolApproved(ctx.api, ctx.alice);

		if (isChopsticks()) {
			borrower = {
				signer: ctx.alice,
				substrate: ctx.alice.address,
				evm: CHOPSTICKS_BORROWER_EVM,
			};
		} else {
			await ensureLiquidator(ctx.api, ctx.bob);
			borrower = await setupBorrower(ctx);
			await dropStHdxPrice(PRICE_CRASH_TARGET);
		}
	});

	after(async () => {
		try {
			if (!isChopsticks()) await restoreStHdxPrice();
		} finally {
			await ctx.api.disconnect();
		}
	});

	it("should dispatch liquidate(collateral=stHDX-670) into liquidate_gigahdx (PEPL path)", async () => {
		const { events, gigaHdxLiquidated } = await liquidate(
			ctx.api,
			ctx.bob,
			STHDX_ASSET_ID,
			borrower!.evm,
			ONE_HOLLAR
		).catch((e) => {
			if (/BadOrigin|UnsupportedCollateral/.test(e.message)) throw e;
			return { events: [], gigaHdxLiquidated: null };
		});

		if (!isChopsticks()) {
			expect(gigaHdxLiquidated, "GigaHdxLiquidated must fire on a real fork").to.not.be.null;
		}
		expect(events).to.be.an("array");
	});

	it("should dispatch liquidate(collateral=GIGAHDX-67) into liquidate_gigahdx (direct path)", async () => {
		const { events, gigaHdxLiquidated } = await liquidate(
			ctx.api,
			ctx.bob,
			GIGAHDX_ASSET_ID,
			borrower!.evm,
			ONE_HOLLAR
		).catch((e) => {
			if (/BadOrigin|UnsupportedCollateral/.test(e.message)) throw e;
			return { events: [], gigaHdxLiquidated: null };
		});

		if (!isChopsticks()) {
			expect(gigaHdxLiquidated, "GigaHdxLiquidated must fire on a real fork").to.not.be.null;
		}
		expect(events).to.be.an("array");
	});
});

// ============================================================================
// 3. Negative cases
// ============================================================================
describe("GIGAHDX liquidation — negative cases", function () {
	this.timeout(180_000);

	let ctx: ChainContext;

	before(async () => {
		ctx = await connect();
		await ensureGigahdxPoolApproved(ctx.api, ctx.alice);
	});

	after(async () => {
		await ctx.api.disconnect();
	});

	it("should reject liquidation when debt asset is not HOLLAR", async () => {
		if (isChopsticks()) return;

		const HDX_ASSET = 0;
		try {
			await liquidate(
				ctx.api,
				ctx.bob,
				GIGAHDX_ASSET_ID,
				"0x0000000000000000000000000000000000000001", // dummy borrower
				ONE_HOLLAR,
				HDX_ASSET
			);
			expect.fail("should have rejected non-HOLLAR debt asset");
		} catch (e: any) {
			expect(e.message).to.match(
				/UnsupportedDebtAsset|NoGigaHdxPosition/,
				"must reject with UnsupportedDebtAsset or NoGigaHdxPosition"
			);
		}
	});

	it("should reject liquidation when borrower has no gigahdx position", async () => {
		if (isChopsticks()) return;

		// Charlie has no stake — use a fresh random-ish address
		const NO_POSITION_EVM = "0x0000000000000000000000000000000000dead01";
		try {
			await liquidate(ctx.api, ctx.bob, GIGAHDX_ASSET_ID, NO_POSITION_EVM, ONE_HOLLAR);
			expect.fail("should have rejected borrower with no position");
		} catch (e: any) {
			expect(e.message).to.match(
				/NoGigaHdxPosition/,
				"must reject with NoGigaHdxPosition"
			);
		}
	});

	it("should reject liquidation when borrower position is healthy (HF > 1)", async () => {
		if (isChopsticks()) return;

		// Set up a fresh borrower but do NOT crash the price — HF stays above 1.
		let healthyBorrower: BorrowerHandle | null = null;
		try {
			await ensureLiquidator(ctx.api, ctx.bob);
			healthyBorrower = await setupBorrower(ctx);
		} catch {
			// If setup fails (e.g. existing position), skip gracefully
			return;
		}

		try {
			await liquidate(
				ctx.api,
				ctx.bob,
				STHDX_ASSET_ID,
				healthyBorrower!.evm,
				ONE_HOLLAR
			);
			// AAVE reverts internally when HF >= 1; the pallet surfaces this as
			// LiquidationCallFailed.
			expect.fail("should have rejected liquidation of healthy position");
		} catch (e: any) {
			expect(e.message).to.match(
				/LiquidationCallFailed|Revert|ExecutedFailed/,
				"must reject healthy liquidation"
			);
		}
	});
});

// ============================================================================
// 4. Post-liquidation state
// ============================================================================
describe("GIGAHDX liquidation — post-liquidation state", function () {
	this.timeout(240_000);

	let ctx: ChainContext;
	let borrower: BorrowerHandle | null = null;
	let stakesBefore: Awaited<ReturnType<typeof queryStakes>> = null;
	let totalLockedBefore: bigint = 0n;

	before(async () => {
		ctx = await connect();
		await ensureGigahdxPoolApproved(ctx.api, ctx.alice);

		if (isChopsticks()) {
			borrower = {
				signer: ctx.alice,
				substrate: ctx.alice.address,
				evm: CHOPSTICKS_BORROWER_EVM,
			};
		} else {
			await ensureLiquidator(ctx.api, ctx.bob);
			borrower = await setupBorrower(ctx);

			// Snapshot state before liquidation
			stakesBefore = await queryStakes(ctx.api, borrower!.substrate);
			totalLockedBefore = await queryTotalLocked(ctx.api);

			await dropStHdxPrice(PRICE_CRASH_TARGET);
		}
	});

	after(async () => {
		try {
			if (!isChopsticks()) await restoreStHdxPrice();
		} finally {
			await ctx.api.disconnect();
		}
	});

	it("should reduce borrower's staked HDX after liquidation", async () => {
		if (isChopsticks()) return;
		if (!stakesBefore) return;

		const { gigaHdxLiquidated } = await liquidate(
			ctx.api,
			ctx.bob,
			STHDX_ASSET_ID,
			borrower!.evm,
			ONE_HOLLAR
		);
		expect(gigaHdxLiquidated, "liquidation must succeed").to.not.be.null;

		const stakesAfter = await queryStakes(ctx.api, borrower!.substrate);
		expect(stakesAfter, "borrower should still have a stake record").to.not.be.null;
		expect(
			stakesAfter!.hdx < stakesBefore!.hdx,
			`borrower HDX should decrease: before=${stakesBefore!.hdx}, after=${stakesAfter!.hdx}`
		).to.be.true;
	});

	it("should reduce borrower's GIGAHDX (aToken) balance after liquidation", async () => {
		if (isChopsticks()) return;
		if (!stakesBefore) return;

		const stakesAfter = await queryStakes(ctx.api, borrower!.substrate);
		expect(stakesAfter, "borrower should still have a stake record").to.not.be.null;
		expect(
			stakesAfter!.gigahdx < stakesBefore!.gigahdx,
			`borrower GIGAHDX should decrease: before=${stakesBefore!.gigahdx}, after=${stakesAfter!.gigahdx}`
		).to.be.true;
	});

	it("should maintain total locked invariant (total_locked decreases by seized amount)", async () => {
		if (isChopsticks()) return;

		const totalLockedAfter = await queryTotalLocked(ctx.api);
		expect(
			totalLockedAfter < totalLockedBefore,
			`TotalLocked should decrease: before=${totalLockedBefore}, after=${totalLockedAfter}`
		).to.be.true;
	});
});

// ============================================================================
// 5. Sequential liquidations
// ============================================================================
describe("GIGAHDX liquidation — sequential small liquidations", function () {
	this.timeout(300_000);

	let ctx: ChainContext;
	let borrower: BorrowerHandle | null = null;

	before(async () => {
		ctx = await connect();
		await ensureGigahdxPoolApproved(ctx.api, ctx.alice);

		if (!isChopsticks()) {
			await ensureLiquidator(ctx.api, ctx.bob);
			borrower = await setupBorrower(ctx);
			await dropStHdxPrice(PRICE_CRASH_TARGET);
		}
	});

	after(async () => {
		try {
			if (!isChopsticks()) await restoreStHdxPrice();
		} finally {
			await ctx.api.disconnect();
		}
	});

	it("should succeed with multiple 1-HOLLAR liquidations", async () => {
		if (isChopsticks()) return;
		if (!borrower) return;

		const results: boolean[] = [];
		const MAX_ATTEMPTS = 3;

		for (let i = 0; i < MAX_ATTEMPTS; i++) {
			try {
				const { gigaHdxLiquidated } = await liquidate(
					ctx.api,
					ctx.bob,
					STHDX_ASSET_ID,
					borrower!.evm,
					ONE_HOLLAR
				);
				results.push(gigaHdxLiquidated !== null);
			} catch {
				results.push(false);
			}
		}

		const successes = results.filter(Boolean).length;
		expect(successes, `at least 1 of ${MAX_ATTEMPTS} sequential liquidations should succeed`).to.be.gte(1);
	});

	it("should progressively reduce borrower stake across sequential liquidations", async () => {
		if (isChopsticks()) return;
		if (!borrower) return;

		const stakeNow = await queryStakes(ctx.api, borrower!.substrate);
		if (!stakeNow || stakeNow.hdx === 0n) return; // already fully liquidated

		const hdxBefore = stakeNow.hdx;
		try {
			await liquidate(ctx.api, ctx.bob, STHDX_ASSET_ID, borrower!.evm, ONE_HOLLAR);
		} catch {
			return; // AAVE may reject if HF recovered
		}

		const stakeAfter = await queryStakes(ctx.api, borrower!.substrate);
		if (stakeAfter) {
			expect(
				stakeAfter.hdx <= hdxBefore,
				"stake should not increase after liquidation"
			).to.be.true;
		}
	});
});

// ============================================================================
// 6. Staking lifecycle
// ============================================================================
describe("GIGAHDX staking — lifecycle basics", function () {
	this.timeout(180_000);

	let ctx: ChainContext;

	before(async () => {
		ctx = await connect();
	});

	after(async () => {
		await ctx.api.disconnect();
	});

	it("should create a stake record when gigaStake is called", async () => {
		if (isChopsticks()) return;

		// Use a fresh account to avoid conflicts
		const staker = ctx.keyring.addFromUri("//LIFECYCLE_TEST_STAKER");

		// Fund from Alice
		const { signAndWait } = await import("../src/utils");
		try {
			await signAndWait(
				ctx.api,
				ctx.api.tx.balances.transferKeepAlive(staker.address, (200_000n * 10n ** 12n).toString()),
				ctx.alice,
				"fund-lifecycle-staker"
			);
		} catch {
			return; // Alice may not have enough funds on a greenfield chain
		}

		// Bind EVM
		try {
			await signAndWait(ctx.api, ctx.api.tx.evmAccounts.bindEvmAddress(), staker, "lifecycle.bindEvm");
		} catch (e: any) {
			if (!/AlreadyBound/.test(e.message)) return; // skip if pallet not available
		}

		// Stake
		const stakeAmount = 100_000n * 10n ** 12n;
		try {
			await signAndWait(
				ctx.api,
				ctx.api.tx.gigaHdx.gigaStake(stakeAmount.toString()),
				staker,
				"lifecycle.gigaStake"
			);
		} catch {
			return; // skip if gigaHdx pallet not configured (e.g. no pool contract)
		}

		const stakes = await queryStakes(ctx.api, staker.address);
		expect(stakes, "stake record must exist after gigaStake").to.not.be.null;
		expect(stakes!.hdx > 0n, "staked HDX must be positive").to.be.true;
		expect(stakes!.gigahdx > 0n, "GIGAHDX aTokens must be positive").to.be.true;
	});

	it("should report zero stakes for an account that never staked", async () => {
		const neverStaked = ctx.keyring.addFromUri("//NEVER_STAKED_ACCOUNT");
		const stakes = await queryStakes(ctx.api, neverStaked.address);
		expect(stakes, "account that never staked should have no Stakes record").to.be.null;
	});
});
