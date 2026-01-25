import { KeyDerivation } from './key-derivation';
import * as bitcoin from 'bitcoinjs-lib';
import * as ecc from 'tiny-secp256k1';
import { createHash } from 'crypto';

bitcoin.initEccLib(ecc);

const ROOT_PUBLIC_KEY =
  "0x044eef776e4f257d68983e45b340c2e9546c5df95447900b6aadfec68fb46fdee257e26b8ba383ddba9914b33c60e869265f859566fff4baef283c54d821ca3b64";
const PALLET_SS58 = '13UVJyLnbVp6z97TrCZZCWMfADQddP3JkNruog9pbUF4Tomm';
const ALICE_HEX_PATH = '0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d';

function compressPubkey(pubKey: string): Buffer {
  const uncompressedHex = pubKey.startsWith("0x") ? pubKey.slice(4) : pubKey;
  const x = Buffer.from(uncompressedHex.slice(0, 64), "hex");
  const y = Buffer.from(uncompressedHex.slice(64), "hex");
  const prefix = y[31] % 2 === 0 ? 0x02 : 0x03;
  return Buffer.concat([Buffer.from([prefix]), x]);
}

function hash160(buffer: Buffer): Buffer {
  const sha256 = createHash('sha256').update(buffer).digest();
  return createHash('ripemd160').update(sha256).digest();
}

function btcAddressFromPubKey(pubKey: string, network: bitcoin.Network): string {
  const compressedPubkey = compressPubkey(pubKey);
  const payment = bitcoin.payments.p2wpkh({
    pubkey: compressedPubkey,
    network,
  });
  return payment.address!;
}

console.log('='.repeat(70));
console.log('DEBUG: Test KeyDerivation Output');
console.log('='.repeat(70));
console.log('');
console.log('Inputs:');
console.log('  ROOT_PUBLIC_KEY:', ROOT_PUBLIC_KEY.slice(0, 40) + '...');
console.log('  PALLET_SS58:', PALLET_SS58);
console.log('  ALICE_HEX_PATH:', ALICE_HEX_PATH);
console.log('');

// Test with polkadot:2034
const derivedPubKey = KeyDerivation.derivePublicKey(
  ROOT_PUBLIC_KEY,
  PALLET_SS58,
  ALICE_HEX_PATH,
  "polkadot:2034"
);

const compressed = compressPubkey(derivedPubKey);
const pubkeyHash = hash160(compressed);
const network = bitcoin.networks.regtest;
const address = btcAddressFromPubKey(derivedPubKey, network);

console.log('Output with polkadot:2034:');
console.log('  Uncompressed pubkey:', derivedPubKey.slice(0, 40) + '...');
console.log('  Compressed pubkey:  ', compressed.toString('hex'));
console.log('  Pubkey hash:        ', pubkeyHash.toString('hex'));
console.log('  Bitcoin address:    ', address);
console.log('');

// Compare with what we expect
console.log('Expected (from earlier verification):');
console.log('  Compressed pubkey:   028a78c0282061fc5f3d1b595b0bbe877b0fcf235744fdab3597e7ac257f736ae9');
console.log('  Pubkey hash:         14f6e14c2f3cf3ff5641a06263c89344a9540e03');
console.log('  Bitcoin address:     bcrt1qznmwznp08nel74jp5p3x8jyngj54grsr99rthj');
console.log('');

// Also show what the OLD UTXOs have
console.log('OLD UTXOs have:');
console.log('  Pubkey hash:         d8ce0f71b94df598efedf183dbdfc8d51410445c');
console.log('  Bitcoin address:     bcrt1qmr8q7udefh6e3mld7xpahh7g652pq3zurswrxz');
