import * as bitcoin from 'bitcoinjs-lib';
import * as ecc from 'tiny-secp256k1';
import { createHash } from 'crypto';

bitcoin.initEccLib(ecc);

// From test output - the pubkey used for verification
const compressedPubkey = Buffer.from('028a78c0282061fc5f3d1b595b0bbe877b0fcf235744fdab3597e7ac257f736ae9', 'hex');

// From test output - the UTXO scriptPubKey
// script: <Buffer 00 14 d8 ce 0f 71 b9 4d f5 98 ef ed f1 83 db df c8 d5 14 10 44 5c>
const utxoScript = Buffer.from('0014d8ce0f71b94df598efedf183dbdfc8d51410445c', 'hex');
const utxoPubkeyHash = utxoScript.slice(2); // Skip OP_0 and push byte

// Compute HASH160 of the pubkey we're signing with
function hash160(buffer: Buffer): Buffer {
  const sha256 = createHash('sha256').update(buffer).digest();
  return createHash('ripemd160').update(sha256).digest();
}

const derivedPubkeyHash = hash160(compressedPubkey);

console.log('Pubkey (compressed):  ', compressedPubkey.toString('hex'));
console.log('HASH160(pubkey):      ', derivedPubkeyHash.toString('hex'));
console.log('UTXO pubkey hash:     ', utxoPubkeyHash.toString('hex'));
console.log('');
console.log('Match:', derivedPubkeyHash.equals(utxoPubkeyHash));

// Also show what Bitcoin addresses these correspond to
const network = bitcoin.networks.regtest;

const addressFromPubkey = bitcoin.payments.p2wpkh({
  pubkey: compressedPubkey,
  network,
}).address;

const addressFromScript = bitcoin.address.fromOutputScript(utxoScript, network);

console.log('');
console.log('Address from pubkey:  ', addressFromPubkey);
console.log('Address from UTXO:    ', addressFromScript);
