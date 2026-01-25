import { ethers } from 'ethers';
import * as ecc from 'tiny-secp256k1';
import { ec as EC } from 'elliptic';

const EPSILON_PREFIX = "sig.network v1.0.0 epsilon derivation";
const MPC_ROOT_KEY = '0xf967622b74b7b619a0f4477118cf39c57c4a179a808526ac6fa1c41372512dad';
const ROOT_PUBLIC_KEY = "0x044eef776e4f257d68983e45b340c2e9546c5df95447900b6aadfec68fb46fdee257e26b8ba383ddba9914b33c60e869265f859566fff4baef283c54d821ca3b64";
const PALLET_SS58 = '13UVJyLnbVp6z97TrCZZCWMfADQddP3JkNruog9pbUF4Tomm';
const ALICE_HEX_PATH = '0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d';
const CHAIN_ID = 'polkadot:2034';
const SECP256K1_N = '0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141';

console.log('='.repeat(70));
console.log('COMPARING DERIVATION METHODS');
console.log('='.repeat(70));
console.log('');

// Build derivation path
const derivationPath = `${EPSILON_PREFIX},${CHAIN_ID},${PALLET_SS58},${ALICE_HEX_PATH}`;
console.log('Derivation path:');
console.log(`  ${derivationPath}`);
console.log('');

// Compute epsilon hash
const hash = ethers.keccak256(ethers.toUtf8Bytes(derivationPath));
console.log('Epsilon hash:', hash);
console.log('');

// Method 1: Test's KeyDerivation (public key point addition)
console.log('--- Method 1: Public key point addition (KeyDerivation) ---');
const ec = new EC("secp256k1");
const uncompressedRoot = ROOT_PUBLIC_KEY.slice(4);
const scalarHex = hash.slice(2);
const x = uncompressedRoot.substring(0, 64);
const y = uncompressedRoot.substring(64);
const oldPoint = ec.curve.point(x, y);
const scalarTimesG = ec.g.mul(scalarHex);
const newPoint = oldPoint.add(scalarTimesG);
const method1Pubkey = `04${newPoint.getX().toString(16).padStart(64, "0")}${newPoint.getY().toString(16).padStart(64, "0")}`;
const method1Compressed = newPoint.encode('hex', true);
console.log('  Compressed pubkey:', method1Compressed);

// Method 2: MPC style (private key scalar addition)
console.log('');
console.log('--- Method 2: Private key scalar addition (MPC style) ---');
const epsilon = BigInt(hash);
const privateKeyBigInt = BigInt(MPC_ROOT_KEY);
const derivedPrivateKey = (privateKeyBigInt + epsilon) % BigInt(SECP256K1_N);
const derivedPrivKeyHex = '0x' + derivedPrivateKey.toString(16).padStart(64, '0');
console.log('  Derived private key:', derivedPrivKeyHex.slice(0, 20) + '...');
const privateKeyBytes = Buffer.from(derivedPrivKeyHex.slice(2), 'hex');
const method2Pubkey = ecc.pointFromScalar(privateKeyBytes, true)!;
console.log('  Compressed pubkey:', Buffer.from(method2Pubkey).toString('hex'));

console.log('');
console.log('--- Comparison ---');
console.log('  Method 1 (point addition):', method1Compressed);
console.log('  Method 2 (scalar addition):', Buffer.from(method2Pubkey).toString('hex'));
console.log('  Match:', method1Compressed === Buffer.from(method2Pubkey).toString('hex'));
