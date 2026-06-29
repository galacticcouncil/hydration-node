import fs from 'fs';
import { TypeRegistry } from '@polkadot/types';
import { xxhashAsHex, blake2AsHex } from '@polkadot/util-crypto';
import { Keyring } from '@polkadot/keyring';
import { hexToU8a, u8aToHex } from '@polkadot/util';

const CHAIN_SPEC = process.argv[2] || 'data/forked-chainspec.json';
const ALICE_PUB = '0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d';

// (asset_id, free_amount_decimal)
const ENDOWMENTS = [
	[2, 1_000_000_000_000_000_000_000n],   // DAI (18 decimals): 1000 DAI
	[5, 100_000_000_000_000_000_000n],     // DOT (10 decimals scaled 10^20 internal): 1000 DOT
	[10, 1_000_000_000_000n],              // USDT (6 decimals): 1M USDT
	[20, 1_000_000_000_000_000_000_000n],  // WETH (18 decimals): 1000 WETH
	[22, 1_000_000_000_000n],              // USDC (6 decimals): 1M USDC
];

const tokensAccountsKey = (account, assetId) => {
	const palletPrefix = xxhashAsHex('Tokens', 128).replace('0x', '');
	const itemPrefix = xxhashAsHex('Accounts', 128).replace('0x', '');
	const accBlake = blake2AsHex(hexToU8a(account), 128).replace('0x', '');
	const accHex = account.replace('0x', '');
	const idBytes = new Uint8Array(4);
	new DataView(idBytes.buffer).setUint32(0, assetId, true);
	const idHex = u8aToHex(idBytes).replace('0x', '');
	const idTwox = xxhashAsHex(idBytes, 64).replace('0x', '');
	return '0x' + palletPrefix + itemPrefix + accBlake + accHex + idTwox + idHex;
};

const encodeAccountData = (free) => {
	const reg = new TypeRegistry();
	reg.register({
		AccountData: {
			free: 'u128',
			reserved: 'u128',
			frozen: 'u128',
		},
	});
	const data = reg.createType('AccountData', { free, reserved: 0, frozen: 0 });
	return u8aToHex(data.toU8a());
};

const main = () => {
	const spec = JSON.parse(fs.readFileSync(CHAIN_SPEC, 'utf8'));
	const top = spec.genesis.raw.top;
	for (const [assetId, free] of ENDOWMENTS) {
		const key = tokensAccountsKey(ALICE_PUB, assetId);
		const value = encodeAccountData(free);
		top[key] = value;
		console.log(`endowed alice asset=${assetId} free=${free}`);
	}
	fs.writeFileSync(CHAIN_SPEC, JSON.stringify(spec));
	console.log(`saved ${CHAIN_SPEC}`);
};

main();
