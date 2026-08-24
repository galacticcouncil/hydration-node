import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
import { hexToU8a } from '@polkadot/util';
import fs from 'fs';

const WASM_PATH = process.env.WASM ||
	'../../target/release/wbuild/hydradx-runtime/hydradx_runtime.compact.compressed.wasm';
const ENDPOINT = process.env.ENDPOINT || 'ws://127.0.0.1:9999';

const main = async () => {
	const wasm = fs.readFileSync(WASM_PATH);
	const api = await ApiPromise.create({ provider: new WsProvider(ENDPOINT) });
	const keyring = new Keyring({ type: 'sr25519' });
	const alice = keyring.addFromUri('//Alice');

	console.log(`endpoint: ${ENDPOINT}`);
	console.log(`wasm: ${WASM_PATH} (${wasm.length} bytes)`);
	console.log(`spec_version pre:  ${(await api.rpc.state.getRuntimeVersion()).specVersion.toString()}`);
	const auth = await api.query.system.authorizedUpgrade();
	console.log(`authorized upgrade: ${auth.toString()}`);

	console.log('submitting system.applyAuthorizedUpgrade ...');
	await new Promise((resolve, reject) => {
		api.tx.system
			.applyAuthorizedUpgrade(`0x${wasm.toString('hex')}`)
			.signAndSend(alice, ({ status, dispatchError }) => {
				if (dispatchError) {
					reject(new Error(dispatchError.toString()));
					return;
				}
				console.log(`status: ${status.type}`);
				if (status.isInBlock || status.isFinalized) resolve();
			})
			.catch(reject);
	});

	console.log('waiting for new spec version...');
	for (let i = 0; i < 60; i++) {
		await new Promise((r) => setTimeout(r, 2000));
		const v = (await api.rpc.state.getRuntimeVersion()).specVersion.toNumber();
		process.stdout.write(`spec=${v} `);
		if (v !== 412) {
			console.log(`\nupgraded to spec_version ${v}`);
			break;
		}
	}
	await api.disconnect();
};

main().catch((e) => {
	console.error(e);
	process.exit(1);
});
