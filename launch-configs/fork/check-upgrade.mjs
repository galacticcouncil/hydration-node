import { ApiPromise, WsProvider } from '@polkadot/api';
const api = await ApiPromise.create({ provider: new WsProvider('ws://127.0.0.1:9999') });

console.log('parachainSystem storage items:');
const sys = api.query.parachainSystem || {};
for (const k of Object.keys(sys)) console.log(' ', k);

const auth = await api.query.system.authorizedUpgrade();
console.log('\nSystem.AuthorizedUpgrade:', auth.toHuman());

const head = await api.rpc.chain.getHeader();
console.log('Best block:', head.number.toNumber());
console.log('Spec version:', (await api.rpc.state.getRuntimeVersion()).specVersion.toNumber());

await api.disconnect();
