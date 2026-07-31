import express from "express";
import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import { cryptoWaitReady } from "@polkadot/util-crypto";

const ENDPOINT = process.env.RPC_ENDPOINT ?? "ws://localhost:9988";
const SPONSOR_URI = process.env.SPONSOR_URI ?? "//Alice";
const PORT = Number(process.env.PORT ?? 3000);

// Policy. A real deployment would load this from config and back it with persistent counters.
const ALLOWED_PALLETS = (process.env.ALLOWED_PALLETS ?? "System,Balances,Currencies,Utility,Omnipool")
	.split(",")
	.map((s) => s.trim());
const MAX_INTENTS_PER_SIGNER = Number(process.env.MAX_INTENTS_PER_SIGNER ?? 20);
const MAX_REF_TIME = BigInt(process.env.MAX_REF_TIME ?? 5_000_000_000n);

const seen = new Map();

function rateLimit(signer) {
	const used = seen.get(signer) ?? 0;
	if (used >= MAX_INTENTS_PER_SIGNER) return false;
	seen.set(signer, used + 1);
	return true;
}

/** Reject anything the sponsor is not willing to pay for, before it costs a fee. */
function screen(api, call) {
	const { section } = api.registry.findMetaCall(call.callIndex);
	if (!ALLOWED_PALLETS.includes(section)) {
		return `pallet '${section}' is not sponsored`;
	}
	return null;
}

async function main() {
	await cryptoWaitReady();

	const api = await ApiPromise.create({ provider: new WsProvider(ENDPOINT) });
	const sponsor = new Keyring({ type: "sr25519" }).addFromUri(SPONSOR_URI);

	console.log(`relayer: connected to ${ENDPOINT}`);
	console.log(`relayer: sponsoring as ${sponsor.address}`);

	const app = express();
	app.use(express.json({ limit: "128kb" }));

	app.get("/health", async (_req, res) => {
		const { data } = await api.query.system.account(sponsor.address);
		res.json({ ok: true, sponsor: sponsor.address, free: data.free.toString() });
	});

	app.get("/nonce/:address", async (req, res) => {
		const nonce = await api.query.metaTx.nonces(req.params.address);
		res.json({ nonce: nonce.toNumber() });
	});

	app.post("/sponsor", async (req, res) => {
		const { signer, call, nonce, deadline, signature } = req.body ?? {};
		if (!signer || !call || nonce === undefined || deadline === undefined || !signature) {
			return res.status(400).json({ error: "signer, call, nonce, deadline and signature are required" });
		}

		let decoded;
		try {
			decoded = api.createType("Call", call);
		} catch {
			return res.status(400).json({ error: "call is not decodable against this runtime" });
		}

		const rejected = screen(api, decoded);
		if (rejected) return res.status(403).json({ error: rejected });
		if (!rateLimit(signer)) return res.status(429).json({ error: "intent quota exhausted for this signer" });

		const onChainNonce = (await api.query.metaTx.nonces(signer)).toNumber();
		if (onChainNonce !== Number(nonce)) {
			return res.status(409).json({ error: `stale nonce: chain expects ${onChainNonce}` });
		}

		const tx = api.tx.metaTx.dispatchMetaTx(signer, decoded, nonce, deadline, signature);

		try {
			// Dry-run first so a doomed intent never costs the sponsor a fee.
			const dry = await tx.dryRun(sponsor);
			if (dry.isErr) {
				return res.status(422).json({ error: "intent would fail", detail: dry.toHuman() });
			}
		} catch (e) {
			console.warn(`relayer: dry-run unavailable (${e.message}), submitting anyway`);
		}

		try {
			const hash = await new Promise((resolve, reject) => {
				tx.signAndSend(sponsor, ({ status, dispatchError }) => {
					if (dispatchError) return reject(new Error(dispatchError.toString()));
					if (status.isInBlock) resolve(status.asInBlock.toHex());
				}).catch(reject);
			});
			res.json({ ok: true, block: hash });
		} catch (e) {
			res.status(500).json({ error: e.message });
		}
	});

	app.listen(PORT, () => console.log(`relayer: listening on :${PORT}`));
}

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
