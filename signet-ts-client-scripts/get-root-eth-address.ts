import { ethers } from 'ethers';
import * as ecc from 'tiny-secp256k1';

const MPC_ROOT_KEY = '0xf967622b74b7b619a0f4477118cf39c57c4a179a808526ac6fa1c41372512dad';

function ethAddressFromPrivateKey(privateKeyHex: string): string {
  const privateKeyBytes = Buffer.from(privateKeyHex.slice(2), 'hex');
  const publicKey = ecc.pointFromScalar(privateKeyBytes, false);
  if (!publicKey) throw new Error('Failed to derive public key');
  // Uncompressed pubkey without prefix (64 bytes)
  const uncompressedPubkey = publicKey.slice(1, 65);
  const hash = ethers.keccak256(uncompressedPubkey);
  return hash.slice(-40); // last 20 bytes = 40 hex chars
}

console.log('MPC Root Key:', MPC_ROOT_KEY);
console.log('');

// Ethereum address from ROOT key directly (no derivation)
const rootEthAddr = ethAddressFromPrivateKey(MPC_ROOT_KEY);
console.log('Ethereum address from ROOT key (no derivation):');
console.log('  0x' + rootEthAddr);
console.log('');
console.log('As byte array for mpc_root_signer_address:');
const bytes = Buffer.from(rootEthAddr, 'hex');
console.log('  [' + Array.from(bytes).map(b => '0x' + b.toString(16).padStart(2, '0')).join(', ') + ']');
