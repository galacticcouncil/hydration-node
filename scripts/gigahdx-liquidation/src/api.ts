import { ApiPromise, Keyring, WsProvider } from "@polkadot/api";
import { cryptoWaitReady } from "@polkadot/util-crypto";
import { WS_URL } from "./constants";

export type KeyringPair = ReturnType<Keyring["addFromUri"]>;

export interface ChainContext {
	api: ApiPromise;
	keyring: Keyring;
	alice: KeyringPair;
	bob: KeyringPair;
}

export async function connect(): Promise<ChainContext> {
	await cryptoWaitReady();
	const api = await ApiPromise.create({ provider: new WsProvider(WS_URL), noInitWarn: true });
	const keyring = new Keyring({ type: "sr25519" });
	return {
		api,
		keyring,
		alice: keyring.addFromUri("//Alice"),
		bob: keyring.addFromUri("//Bob"),
	};
}
