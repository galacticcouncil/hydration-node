import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import { cryptoWaitReady, mnemonicGenerate } from "@polkadot/util-crypto";
import { buildIntent } from "./payload.js";

const ENDPOINT = process.env.RPC_ENDPOINT ?? "ws://localhost:9988";
const RELAYER = process.env.RELAYER_URL ?? "http://localhost:3000";

async function main() {
	await cryptoWaitReady();
	const api = await ApiPromise.create({ provider: new WsProvider(ENDPOINT) });

	// A wallet that has never held a token and cannot pay for anything.
	const user = new Keyring({ type: "sr25519" }).addFromUri(mnemonicGenerate());
	const { data } = await api.query.system.account(user.address);
	console.log(`user    ${user.address}`);
	console.log(`balance ${data.free.toString()}  (expected 0)`);

	const call = api.tx.system.remarkWithEvent("sponsored by the relayer");
	const intent = await buildIntent(api, user, call);
	console.log(`intent  nonce=${intent.nonce} deadline=${intent.deadline}`);

	const response = await fetch(`${RELAYER}/sponsor`, {
		method: "POST",
		headers: { "content-type": "application/json" },
		body: JSON.stringify(intent),
	});
	const body = await response.json();
	console.log(`relayer ${response.status}`, body);

	if (!response.ok) process.exit(1);

	const after = await api.query.system.account(user.address);
	const nonce = await api.query.metaTx.nonces(user.address);
	console.log(`balance ${after.data.free.toString()}  (still 0 — the relayer paid)`);
	console.log(`nonce   ${nonce.toNumber()}  (consumed, so the intent cannot be replayed)`);

	await api.disconnect();
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
