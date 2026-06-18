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

describe('signet feeless respond (allowlist-gated)', () => {
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

		// Fund both with exactly the existential deposit — no gas buffer.
		await setNativeBalance(provider, signer.address, ED);
		await setNativeBalance(provider, outsider.address, ED);
	}, 600000);

	afterAll(async () => {
		await api?.disconnect();
	});

	it('add_signer should have authorized the signer', async () => {
		const entry = await api.query.signet.signers(signer.address);
		expect((entry as any).isSome).toBe(true);
	});

	it('respond should be annotated Pays::No (zero partial fee)', async () => {
		const info = await api.tx.signet.respond([REQUEST_ID], [SIGNATURE]).paymentInfo(signer.address);
		expect((info as any).partialFee.toBigInt()).toBe(0n);
	});

	it('respond should succeed for an ED-only signer and not charge a fee', async () => {
		const before = await freeBalance(api, signer.address);

		const events = await submitAndMine(api, provider, api.tx.signet.respond([REQUEST_ID], [SIGNATURE]), signer);

		expect(findEvent(events, 'signet', 'SignatureResponded')).toBeDefined();
		expect(findEvent(events, 'system', 'ExtrinsicFailed')).toBeUndefined();

		const after = await freeBalance(api, signer.address);
		expect(after).toBe(before);
		expect(after).toBe(ED);
	}, 300000);

	it('respond should be rejected for a non-allowlisted account', async () => {
		const events = await submitAndMine(api, provider, api.tx.signet.respond([REQUEST_ID], [SIGNATURE]), outsider);

		const failed = findEvent(events, 'system', 'ExtrinsicFailed');
		expect(failed).toBeDefined();

		const err = moduleErrorName(api, failed);
		expect(err.section).toBe('signet');
		expect(err.name).toBe('NotAuthorizedSigner');
	}, 300000);
});
