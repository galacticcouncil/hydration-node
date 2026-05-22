import { connect } from "./src/api";
import { BORROWER_URI } from "./src/constants";

async function main() {
	const ctx = await connect();
	const borrower = ctx.keyring.addFromUri(BORROWER_URI);
	const stake = (await ctx.api.query.gigaHdx.stakes(borrower.address)) as any;
	console.log("borrower stake (raw):", stake.toString());
	console.log("borrower stake (human):", stake.toHuman());
	console.log("isEmpty:", stake.isEmpty);
	if (!stake.isEmpty) {
		const s = stake.unwrap();
		console.log("  hdx:", s.hdx.toString());
		console.log("  gigahdx:", s.gigahdx.toString());
	}

	// Bob nonce
	const bobNonce = await ctx.api.rpc.system.accountNextIndex(ctx.bob.address);
	console.log("bob nonce:", bobNonce.toString());
	const bobAcc = await ctx.api.query.system.account(ctx.bob.address);
	console.log("bob system acct:", JSON.stringify(bobAcc.toHuman()));

	await ctx.api.disconnect();
}
main().catch(e => { console.error("FAIL:", e); process.exit(1); });
