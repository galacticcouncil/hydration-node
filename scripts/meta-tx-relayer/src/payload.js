import { u8aConcat, stringToU8a } from "@polkadot/util";
import { blake2AsU8a, decodeAddress } from "@polkadot/util-crypto";

// Must match PAYLOAD_TAG in pallets/meta-tx/src/lib.rs.
const PAYLOAD_TAG = "hydra-metatx";

/** Chain constants the payload is bound to. Read once per connection. */
export async function domain(api) {
	return {
		// The runtime reads System::block_hash(0), which is the genesis hash once block 1 exists.
		genesisHash: (await api.query.system.blockHash(0)).toU8a(),
		specVersion: api.runtimeVersion.specVersion.toNumber(),
	};
}

/** The 32 bytes a signer must sign to authorise `call`. Mirrors Pallet::signing_payload. */
export function signingPayload(api, { call, signer, nonce, deadline, genesisHash, specVersion }) {
	return blake2AsU8a(
		u8aConcat(
			stringToU8a(PAYLOAD_TAG),
			call.toU8a(),
			decodeAddress(signer),
			api.createType("u32", nonce).toU8a(),
			api.createType("u32", deadline).toU8a(),
			genesisHash,
			api.createType("u32", specVersion).toU8a()
		),
		256
	);
}

/** Build and sign an intent with a keyring pair. Returns the body the relayer expects. */
export async function buildIntent(api, pair, call, { lifetimeBlocks = 100 } = {}) {
	const { genesisHash, specVersion } = await domain(api);
	const nonce = (await api.query.metaTx.nonces(pair.address)).toNumber();
	const head = await api.rpc.chain.getHeader();
	const deadline = head.number.toNumber() + lifetimeBlocks;

	const payload = signingPayload(api, {
		call,
		signer: pair.address,
		nonce,
		deadline,
		genesisHash,
		specVersion,
	});

	return {
		signer: pair.address,
		call: call.method.toHex(),
		nonce,
		deadline,
		signature: api.createType("MultiSignature", { Sr25519: pair.sign(payload) }).toHex(),
	};
}
