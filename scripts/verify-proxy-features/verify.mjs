import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import { u8aToHex } from "@polkadot/util";
import { cryptoWaitReady } from "@polkadot/util-crypto";

const RPC = process.env.RPC || "wss://4.lark.hydration.cloud";
const SPAWNER_SURI = process.env.SPAWNER_SURI || "//Alice";
const DELEGATE_SURI = process.env.DELEGATE_SURI || "//Bob";
const PURE_SPAWNER_SURI = process.env.PURE_SPAWNER_SURI || "//Charlie";
const FUND_AMOUNT = 10_000_000_000_000_000n; // 10,000 HDX (covers ED + proxy deposits)

function bail(msg) {
	console.error(`\n✗ ${msg}`);
	process.exit(1);
}

function sendAndWait(tx, signer) {
	return new Promise((resolve, reject) => {
		tx.signAndSend(signer, ({ status, events, dispatchError }) => {
			if (dispatchError) {
				if (dispatchError.isModule) {
					const { section, name } = dispatchError.registry.findMetaError(dispatchError.asModule);
					return reject(new Error(`${section}.${name}`));
				}
				return reject(new Error(dispatchError.toString()));
			}
			if (status.isInBlock) {
				resolve({ blockHash: status.asInBlock, events });
			}
		}).catch(reject);
	});
}

async function ensureFunded(api, from, target, minBalance) {
	const { data } = await api.query.system.account(target);
	if (data.free.toBigInt() >= minBalance) return;
	const topUp = minBalance - data.free.toBigInt();
	console.log(`  topping up ${target} with ${topUp} (from ${from.address})`);
	await sendAndWait(api.tx.balances.transferKeepAlive(target, topUp), from);
}

async function main() {
	await cryptoWaitReady();
	const api = await ApiPromise.create({ provider: new WsProvider(RPC) });

	const version = api.runtimeVersion;
	console.log(`Connected to ${RPC}`);
	console.log(`Runtime: ${version.specName.toString()}/${version.specVersion.toString()}`);
	if (version.specVersion.toNumber() < 411) {
		bail(`Expected spec_version >= 411, got ${version.specVersion.toString()}`);
	}

	const ss58 = api.registry.chainSS58 ?? 42;
	const keyring = new Keyring({ type: "sr25519", ss58Format: ss58 });
	const alice = keyring.addFromUri(SPAWNER_SURI);
	const bob = keyring.addFromUri(DELEGATE_SURI);
	const charlie = keyring.addFromUri(PURE_SPAWNER_SURI);
	const pkHex = (a) => u8aToHex(a.publicKey);

	console.log("\n[1/3] Reverse proxy lookup: proxyApi.proxiesForDelegate");
	console.log(`  addProxy: ${alice.address} -> delegate ${bob.address} (Any, delay=0)`);
	let addedProxy = true;
	try {
		await sendAndWait(api.tx.proxy.addProxy(bob.address, "Any", 0), alice);
	} catch (err) {
		if (String(err.message).includes("proxy.Duplicate")) {
			console.log("  (proxy already exists, reusing)");
			addedProxy = false;
		} else {
			throw err;
		}
	}

	const proxies = await api.call.proxyApi.proxiesForDelegate(bob.address);
	console.log(`  proxiesForDelegate returned ${proxies.length} entry(ies)`);
	const alicePk = pkHex(alice);
	const hit = proxies.find((t) => u8aToHex(t[0].toU8a()) === alicePk);
	if (!hit) {
		bail(`Alice not found in reverse lookup for Bob. Returned: ${proxies.toString()}`);
	}
	console.log(`  ✓ delegator=${hit[0].toString()} type=${hit[1].toString()} delay=${hit[2].toString()}`);

	if (addedProxy) {
		console.log("  cleanup: removeProxy");
		await sendAndWait(api.tx.proxy.removeProxy(bob.address, "Any", 0), alice);
	}

	console.log("\n[2/3] Pure proxy creation metadata: proxy.pureProxyCreationInfo");
	await ensureFunded(api, alice, charlie.address, FUND_AMOUNT);

	console.log(`  createPure: spawner=${charlie.address} (Any, delay=0, index=0)`);
	const { events: createEvents } = await sendAndWait(api.tx.proxy.createPure("Any", 0, 0), charlie);
	const pureEvent = createEvents.find((r) => api.events.proxy.PureCreated.is(r.event));
	if (!pureEvent) bail("PureCreated event not found");
	const pureAddr = pureEvent.event.data.pure.toString();
	console.log(`  pure address: ${pureAddr}`);

	const info = await api.query.proxy.pureProxyCreationInfo(pureAddr);
	if (info.isNone) bail("pureProxyCreationInfo is None — storage not populated");
	const rec = info.unwrap();
	console.log(
		`  ✓ record: spawner=${rec.spawner.toString()} type=${rec.proxyType.toString()} ` +
			`index=${rec.index.toString()} height=${rec.height.toString()} ext_index=${rec.extIndex.toString()}`
	);
	if (u8aToHex(rec.spawner.toU8a()) !== pkHex(charlie)) {
		bail(`spawner mismatch: want ${charlie.address}, got ${rec.spawner.toString()}`);
	}

	console.log("  cleanup: killPure via proxy.proxy using stored metadata");
	const killCall = api.tx.proxy.killPure(rec.spawner, rec.proxyType, rec.index, rec.height, rec.extIndex);
	await sendAndWait(api.tx.proxy.proxy(pureAddr, null, killCall), charlie);

	const after = await api.query.proxy.pureProxyCreationInfo(pureAddr);
	if (after.isSome) bail("pureProxyCreationInfo still present after kill_pure");
	console.log("  ✓ creation info cleared after kill_pure");

	console.log("\n[3/3] Proxy fee extrinsic: dispatcher.dispatchWithFeePayer");
	if (!api.tx.dispatcher?.dispatchWithFeePayer) {
		bail("dispatcher.dispatchWithFeePayer missing from metadata");
	}
	const meta = api.tx.dispatcher.dispatchWithFeePayer.meta;
	console.log(`  ✓ present; args: ${meta.args.map((a) => a.name.toString()).join(", ")}`);
	console.log("  (for full EVM gas routing e2e: scripts/proxy-fee-test/)");

	console.log("\n✓ All checks passed on lark 4");
	await api.disconnect();
	process.exit(0);
}

main().catch((err) => {
	console.error(err);
	process.exit(1);
});
