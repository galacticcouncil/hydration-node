import { ethers } from 'ethers';
import * as ecc from 'tiny-secp256k1';

const CONFIG = {
  EPSILON_DERIVATION_PREFIX: 'sig.network v1.0.0 epsilon derivation',
  SECP256K1_N: '0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141',
};

const MPC_ROOT_KEY = '0xf967622b74b7b619a0f4477118cf39c57c4a179a808526ac6fa1c41372512dad';
const PALLET_SS58 = '13UVJyLnbVp6z97TrCZZCWMfADQddP3JkNruog9pbUF4Tomm';
const ALICE_HEX_PATH = '0xd43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d';

function derivePrivateKey(path: string, sender: string, basePrivateKey: string, chainId: string): string {
  const derivationPath = `${CONFIG.EPSILON_DERIVATION_PREFIX},${chainId},${sender},${path}`;
  console.log(`  Derivation path: ${derivationPath}`);
  const hash = ethers.keccak256(ethers.toUtf8Bytes(derivationPath));
  const epsilon = BigInt(hash);
  const privateKeyBigInt = BigInt(basePrivateKey);
  const derivedPrivateKey = (privateKeyBigInt + epsilon) % BigInt(CONFIG.SECP256K1_N);
  return '0x' + derivedPrivateKey.toString(16).padStart(64, '0');
}

console.log('MPC shows derived private key starts with: 0x77921eab88...');
console.log('');

// Try with different chainIds
const chainIds = [
  'polkadot:2034',
  'bip122:000000000933ea01ad0ee984209779ba',
  'polkadot:2034', // without leading 0
];

for (const chainId of chainIds) {
  console.log(`\n--- ChainId: ${chainId} ---`);
  const derivedKey = derivePrivateKey(ALICE_HEX_PATH, PALLET_SS58, MPC_ROOT_KEY, chainId);
  console.log(`  Derived key: ${derivedKey}`);
  console.log(`  Starts with 0x77921eab88: ${derivedKey.startsWith('0x77921eab88')}`);
}

// Also try with the UTXO's scriptPubKey address to see if that gives us a clue
// The UTXO was at bcrt1qmr8q7udefh6e3mld7xpahh7g652pq3zurswrxz = d8ce0f71b94df598efedf183dbdfc8d51410445c

// Try different orderings of parameters
console.log('\n--- Trying different parameter orderings ---');

// Maybe MPC uses path,sender instead of sender,path?
const altPath1 = `${CONFIG.EPSILON_DERIVATION_PREFIX},polkadot:2034,${ALICE_HEX_PATH},${PALLET_SS58}`;
console.log(`Path: ${altPath1}`);
const altHash1 = ethers.keccak256(ethers.toUtf8Bytes(altPath1));
const altKey1 = (BigInt(MPC_ROOT_KEY) + BigInt(altHash1)) % BigInt(CONFIG.SECP256K1_N);
console.log(`Derived key: 0x${altKey1.toString(16).padStart(64, '0')}`);
