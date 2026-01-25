import { ethers } from 'ethers';
import * as bitcoin from 'bitcoinjs-lib';
import * as ecc from 'tiny-secp256k1';

bitcoin.initEccLib(ecc);

const CONFIG = {
  EPSILON_DERIVATION_PREFIX: 'sig.network v1.0.0 epsilon derivation',
  SECP256K1_N: '0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141',
};

const MPC_ROOT_KEY = '0xf967622b74b7b619a0f4477118cf39c57c4a179a808526ac6fa1c41372512dad';
const PALLET_SS58 = '13UVJyLnbVp6z97TrCZZCWMfADQddP3JkNruog9pbUF4Tomm';
const ROOT_PATH = 'root';
const CHAIN_ID = 'polkadot:2034';

// Derive private key using the same method as MPC server
function derivePrivateKey(path: string, sender: string, basePrivateKey: string, chainId: string): string {
  const derivationPath = `${CONFIG.EPSILON_DERIVATION_PREFIX},${chainId},${sender},${path}`;
  const hash = ethers.keccak256(ethers.toUtf8Bytes(derivationPath));
  const epsilon = BigInt(hash);
  const privateKeyBigInt = BigInt(basePrivateKey);
  const derivedPrivateKey = (privateKeyBigInt + epsilon) % BigInt(CONFIG.SECP256K1_N);
  return '0x' + derivedPrivateKey.toString(16).padStart(64, '0');
}

function ethAddressFromPrivateKey(privateKeyHex: string): Uint8Array {
  const privateKeyBytes = Buffer.from(privateKeyHex.slice(2), 'hex');
  const publicKey = ecc.pointFromScalar(privateKeyBytes, false);
  if (!publicKey) throw new Error('Failed to derive public key');
  // Uncompressed pubkey without prefix (64 bytes)
  const uncompressedPubkey = publicKey.slice(1, 65);
  const hash = ethers.keccak256(uncompressedPubkey);
  return new Uint8Array(Buffer.from(hash.slice(2), 'hex').slice(-20));
}

async function main() {
  console.log('='.repeat(70));
  console.log('VERIFYING ALL ADDRESS DERIVATIONS');
  console.log('='.repeat(70));

  // Derive private key at path "root"
  const derivedPrivateKey = derivePrivateKey(ROOT_PATH, PALLET_SS58, MPC_ROOT_KEY, CHAIN_ID);
  console.log('\n1. Derived private key at path "root":');
  console.log('   Private key:', derivedPrivateKey);

  // Get public key from derived private key
  const privateKeyBytes = Buffer.from(derivedPrivateKey.slice(2), 'hex');
  const uncompressedPubKey = ecc.pointFromScalar(privateKeyBytes, false)!;
  const compressedPubKey = ecc.pointFromScalar(privateKeyBytes, true)!;
  
  console.log('\n2. Derived public keys:');
  console.log('   Uncompressed:', Buffer.from(uncompressedPubKey).toString('hex'));
  console.log('   Compressed:  ', Buffer.from(compressedPubKey).toString('hex'));

  // Ethereum address from derived key (CORRECT for mpc_root_signer_address)
  const derivedEthAddr = ethAddressFromPrivateKey(derivedPrivateKey);
  console.log('\n3. Ethereum address from derived key (for mpc_root_signer_address):');
  console.log('   Address:', Buffer.from(derivedEthAddr).toString('hex'));

  // Bitcoin P2WPKH address from derived key
  const { address, output } = bitcoin.payments.p2wpkh({
    pubkey: Buffer.from(compressedPubKey),
    network: bitcoin.networks.regtest,
  });
  const btcPubkeyHash = output!.slice(2);
  console.log('\n4. Bitcoin P2WPKH from derived key (for TESTNET_VAULT_ADDRESS):');
  console.log('   Address:', address);
  console.log('   Pubkey hash:', btcPubkeyHash.toString('hex'));

  // Compare with what we set in pallet
  const TESTNET_VAULT_ADDRESS_HASH = Buffer.from([
    0x67, 0x6c, 0x36, 0x49, 0xc4, 0xa4, 0x24, 0x7e, 0xd0, 0x14, 0xff, 0x52, 0x50,
    0x73, 0xe6, 0x3b, 0xba, 0x5c, 0x43, 0xe4,
  ]);

  console.log('\n' + '='.repeat(70));
  console.log('SUMMARY:');
  console.log('='.repeat(70));
  
  console.log('\nBitcoin vault address (TESTNET_VAULT_ADDRESS):');
  console.log('  Derived:  ', btcPubkeyHash.toString('hex'));
  console.log('  In pallet:', TESTNET_VAULT_ADDRESS_HASH.toString('hex'));
  console.log('  Match:    ', btcPubkeyHash.equals(TESTNET_VAULT_ADDRESS_HASH) ? '✅ CORRECT' : '❌ WRONG');

  console.log('\nmpc_root_signer_address (for signature verification):');
  console.log('  Should be:', Buffer.from(derivedEthAddr).toString('hex'));
  
  console.log('\n' + '='.repeat(70));
  console.log('FOR VAULT INITIALIZATION, use this 20-byte array:');
  console.log('='.repeat(70));
  console.log('  [' + Array.from(derivedEthAddr).map(b => '0x' + b.toString(16).padStart(2, '0')).join(', ') + ']');
}

main().catch(console.error);
