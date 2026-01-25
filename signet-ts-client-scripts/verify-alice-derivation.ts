import { ethers } from 'ethers';
import * as ecc from 'tiny-secp256k1';
import { ec as EC } from 'elliptic';

const CONFIG = {
  EPSILON_DERIVATION_PREFIX: 'sig.network v1.0.0 epsilon derivation',
  SECP256K1_N: '0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141',
};

const MPC_ROOT_KEY = '0xf967622b74b7b619a0f4477118cf39c57c4a179a808526ac6fa1c41372512dad';
const ROOT_PUBLIC_KEY = "0x044eef776e4f257d68983e45b340c2e9546c5df95447900b6aadfec68fb46fdee257e26b8ba383ddba9914b33c60e869265f859566fff4baef283c54d821ca3b64";

const PALLET_SS58 = '13UVJyLnbVp6z97TrCZZCWMfADQddP3JkNruog9pbUF4Tomm';
const ALICE_HEX_PATH = '0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d';
const CHAIN_ID = 'bip122:000000000933ea01ad0ee984209779ba';

// MPC derivation (using private key)
function derivePrivateKey(path: string, sender: string, basePrivateKey: string, chainId: string): string {
  const derivationPath = `${CONFIG.EPSILON_DERIVATION_PREFIX},${chainId},${sender},${path}`;
  console.log('Derivation path:', derivationPath);
  const hash = ethers.keccak256(ethers.toUtf8Bytes(derivationPath));
  console.log('Epsilon hash:', hash);
  const epsilon = BigInt(hash);
  const privateKeyBigInt = BigInt(basePrivateKey);
  const derivedPrivateKey = (privateKeyBigInt + epsilon) % BigInt(CONFIG.SECP256K1_N);
  return '0x' + derivedPrivateKey.toString(16).padStart(64, '0');
}

// Test's KeyDerivation (using public key with point addition)
function derivePublicKey(rootPublicKey: string, predecessorId: string, path: string, chainId: string): string {
  const ec = new EC("secp256k1");
  const EPSILON_PREFIX = "sig.network v1.0.0 epsilon derivation";

  const uncompressedRoot = rootPublicKey.slice(4);

  const derivationPath = `${EPSILON_PREFIX},${chainId},${predecessorId},${path}`;
  console.log('Test derivation path:', derivationPath);
  const hash = ethers.keccak256(ethers.toUtf8Bytes(derivationPath));
  console.log('Test epsilon hash:', hash);
  const scalarHex = hash.slice(2);

  const x = uncompressedRoot.substring(0, 64);
  const y = uncompressedRoot.substring(64);

  const oldPoint = ec.curve.point(x, y);
  const scalarTimesG = ec.g.mul(scalarHex);
  const newPoint = oldPoint.add(scalarTimesG);

  return `0x04${newPoint.getX().toString(16).padStart(64, "0")}${newPoint.getY().toString(16).padStart(64, "0")}`;
}

console.log('='.repeat(70));
console.log('VERIFYING ALICE DERIVATION');
console.log('='.repeat(70));
console.log('');
console.log('Parameters:');
console.log('  MPC_ROOT_KEY:', MPC_ROOT_KEY);
console.log('  PALLET_SS58:', PALLET_SS58);
console.log('  ALICE_HEX_PATH:', ALICE_HEX_PATH);
console.log('  CHAIN_ID:', CHAIN_ID);
console.log('');

console.log('--- MPC derivation (private key method) ---');
const derivedPrivateKey = derivePrivateKey(ALICE_HEX_PATH, PALLET_SS58, MPC_ROOT_KEY, CHAIN_ID);
console.log('Derived private key:', derivedPrivateKey);

// Get public key from derived private key
const privateKeyBytes = Buffer.from(derivedPrivateKey.slice(2), 'hex');
const mpcDerivedUncompressed = ecc.pointFromScalar(privateKeyBytes, false)!;
const mpcDerivedCompressed = ecc.pointFromScalar(privateKeyBytes, true)!;
console.log('MPC derived pubkey (uncompressed):', '0x' + Buffer.from(mpcDerivedUncompressed).toString('hex'));
console.log('MPC derived pubkey (compressed):  ', Buffer.from(mpcDerivedCompressed).toString('hex'));

console.log('');
console.log('--- Test derivation (public key point addition method) ---');
const testDerivedPubKey = derivePublicKey(ROOT_PUBLIC_KEY, PALLET_SS58, ALICE_HEX_PATH, CHAIN_ID);
console.log('Test derived pubkey (uncompressed):', testDerivedPubKey);

// Compress test pubkey
const ec = new EC("secp256k1");
const testUncompressed = testDerivedPubKey.slice(4);
const testX = testUncompressed.substring(0, 64);
const testY = testUncompressed.substring(64);
const testPoint = ec.curve.point(testX, testY);
const testCompressed = testPoint.encode('hex', true);
console.log('Test derived pubkey (compressed):  ', testCompressed);

console.log('');
console.log('--- Comparison ---');
const match = Buffer.from(mpcDerivedCompressed).toString('hex') === testCompressed;
console.log('Compressed pubkeys match:', match);

if (!match) {
  console.log('');
  console.log('MISMATCH! Expected:', Buffer.from(mpcDerivedCompressed).toString('hex'));
  console.log('          Got:     ', testCompressed);
}
