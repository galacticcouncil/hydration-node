import { connect } from "./src/api";
import { ensureGigahdxPoolApproved } from "./src/governance";
import { ensureLiquidator } from "./src/liquidator";
import { setupBorrower } from "./src/borrower";
import { dropStHdxPrice } from "./src/oracle";

async function main() {
	console.log("[1] connect");
	const ctx = await connect();
	console.log("  ok, current block:", (await ctx.api.rpc.chain.getHeader()).number.toNumber());

	console.log("[2] ensureGigahdxPoolApproved");
	await ensureGigahdxPoolApproved(ctx.api, ctx.alice);
	console.log("  ok");

	console.log("[3] ensureLiquidator");
	await ensureLiquidator(ctx.api, ctx.bob);
	console.log("  ok");

	console.log("[4] setupBorrower");
	const b = await setupBorrower(ctx);
	console.log("  ok:", b.evm);

	console.log("[5] dropStHdxPrice");
	await dropStHdxPrice();
	console.log("  ok");

	await ctx.api.disconnect();
	console.log("DONE");
}
main().catch(e => { console.error("FAIL:", e.message); process.exit(1); });
