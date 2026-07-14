import { ApiPromise, WsProvider } from '@polkadot/api';
import { Keyring } from '@polkadot/keyring';
import type { KeyringPair } from '@polkadot/keyring/types';
import {
	createApi,
	ED,
	executeAsRootViaScheduler,
	findEvent,
	freeBalance,
	moduleErrorName,
	setNativeBalance,
	submitAndMine,
} from './utils';

// Arbitrary request id + dummy signature (not verified on-chain).
const REQUEST_ID = '0x' + '11'.repeat(32);
const SIGNATURE = {
	bigR: { x: '0x' + '01'.repeat(32), y: '0x' + '02'.repeat(32) },
	s: '0x' + '03'.repeat(32),
	recoveryId: 0,
};

// Existential deposit plus enough headroom to lock one respond fee.
const FUND = ED + 10_000_000_000_000n;

describe('signet respond (lock / refund-on-success)', () => {
	let api: ApiPromise;
	let provider: WsProvider;
	let signer: KeyringPair;
	let outsider: KeyringPair;

	beforeAll(async () => {
		({ api, provider } = await createApi());
		const keyring = new Keyring({ type: 'sr25519' });
		signer = keyring.addFromUri('//signetSigner');
		outsider = keyring.addFromUri('//signetOutsider');

		await executeAsRootViaScheduler(api, provider, api.tx.signet.addSigner(signer.address));

		await setNativeBalance(provider, signer.address, FUND);
		await setNativeBalance(provider, outsider.address, FUND);
	}, 600000);

	afterAll(async () => {
		await api?.disconnect();
	});

	it('add_signer should have authorized the signer', async () => {
		const entry = await api.query.signet.signers(signer.address);
		expect((entry as any).isSome).toBe(true);
	});

	it('respond should predict a non-zero fee (locked upfront)', async () => {
		const info = await api.tx.signet.respond([REQUEST_ID], [SIGNATURE]).paymentInfo(signer.address);
		expect((info as any).partialFee.toBigInt()).toBeGreaterThan(0n);
	});

	it('respond should refund the fee when successful', async () => {
		const before = await freeBalance(api, signer.address);

		const events = await submitAndMine(api, provider, api.tx.signet.respond([REQUEST_ID], [SIGNATURE]), signer);

		expect(findEvent(events, 'signet', 'SignatureResponded')).toBeDefined();
		expect(findEvent(events, 'system', 'ExtrinsicFailed')).toBeUndefined();

		const after = await freeBalance(api, signer.address);
		expect(after).toBe(before);
	}, 300000);

	it('respond should charge the fee when the caller is not allowlisted', async () => {
		const before = await freeBalance(api, outsider.address);

		const events = await submitAndMine(api, provider, api.tx.signet.respond([REQUEST_ID], [SIGNATURE]), outsider);

		const failed = findEvent(events, 'system', 'ExtrinsicFailed');
		expect(failed).toBeDefined();

		const err = moduleErrorName(api, failed);
		expect(err.section).toBe('signet');
		expect(err.name).toBe('NotAuthorizedSigner');

		const after = await freeBalance(api, outsider.address);
		expect(after).toBeLessThan(before);
	}, 300000);
});
