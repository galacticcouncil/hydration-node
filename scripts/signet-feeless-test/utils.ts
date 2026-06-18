import { ApiPromise, WsProvider } from '@polkadot/api';

export const ENDPOINT = process.env.WS_URL || 'ws://localhost:8000';

// HDX existential deposit (1 HDX, 12 decimals).
export const ED = 1_000_000_000_000n;

export async function createApi(): Promise<{ api: ApiPromise; provider: WsProvider }> {
	// Big request timeout: the first dev_newBlock on a fresh fork can exceed 60s.
	const provider = new WsProvider(ENDPOINT, 2_500, {}, 600_000);
	const api = await ApiPromise.create({ provider, throwOnConnect: false });
	await api.isReady;
	return { api, provider };
}

export async function newBlock(provider: WsProvider, count = 1): Promise<void> {
	await provider.send('dev_newBlock', [{ count }]);
}

// Dispatch `call` as Root via the scheduler (chopsticks dev_setStorage).
export async function executeAsRootViaScheduler(api: ApiPromise, provider: WsProvider, call: any): Promise<void> {
	const header = await api.rpc.chain.getHeader();
	const at = header.number.toNumber() + 1;
	const callHex = call.method.toHex();

	await provider.send('dev_setStorage', [
		{
			Scheduler: {
				Agenda: [[[at], [{ call: { Inline: callHex }, origin: { system: 'Root' } }]]],
			},
		},
	]);
	await newBlock(provider);
}

// Make the account exist (providers = 1) with exactly `free` balance.
export async function setNativeBalance(provider: WsProvider, address: string, free: bigint): Promise<void> {
	await provider.send('dev_setStorage', [
		{
			System: {
				Account: [[[address], { providers: 1, data: { free: free.toString() } }]],
			},
		},
	]);
}

export async function freeBalance(api: ApiPromise, address: string): Promise<bigint> {
	const acc = await api.query.system.account(address);
	return (acc as any).data.free.toBigInt();
}

export async function submitAndMine(api: ApiPromise, provider: WsProvider, tx: any, signer: any): Promise<any[]> {
	await tx.signAndSend(signer);
	await newBlock(provider);
	const blockHash = await api.rpc.chain.getBlockHash();
	const events = await api.query.system.events.at(blockHash);
	return (events as any).toArray();
}

export function findEvent(events: any[], section: string, method: string): any | undefined {
	return events.find((r) => r.event.section === section && r.event.method === method);
}

export function moduleErrorName(api: ApiPromise, failedEvent: any): { section: string; name: string } {
	const dispatchError = failedEvent.event.data[0];
	const meta = api.registry.findMetaError(dispatchError.asModule);
	return { section: meta.section, name: meta.name };
}
