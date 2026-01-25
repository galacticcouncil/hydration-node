import { ethers } from 'ethers';
import * as bitcoin from 'bitcoinjs-lib';
import * as ecc from 'tiny-secp256k1';

// Initialize ECC for bitcoinjs-lib
bitcoin.initEccLib(ecc);

const CONFIG = {
  EPSILON_DERIVATION_PREFIX: 'sig.network v1.0.0 epsilon derivation',
  SECP256K1_N: '0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141',
};

// MPC Root Key from .env
const MPC_ROOT_KEY = '0xf967622b74b7b619a0f4477118cf39c57c4a179a808526ac6fa1c41372512dad';

// Expected vault address from pallet (TESTNET_VAULT_ADDRESS) - UPDATED
const EXPECTED_VAULT_ADDRESS_HASH = Buffer.from([
  0x67, 0x6c, 0x36, 0x49, 0xc4, 0xa4, 0x24, 0x7e, 0xd0, 0x14,
  0xff, 0x52, 0x50, 0x73, 0xe6, 0x3b, 0xba, 0x5c, 0x43, 0xe4
]);

// btcVault pallet account (derived from PalletId b"py/btcvt")
// This is what the sender would be for withdrawals
const BTC_VAULT_PALLET_ACCOUNT = '13UVJyLnbVp6z97TrCZZCWMfADQddP3JkNruog9pbUF4Tomm';

// Path for withdrawals
const ROOT_PATH = 'root';

// Chain ID used for key derivation
const CHAIN_ID = 'polkadot:2034';

function deriveEpsilonWithChainId(requester: string, path: string, chainId: string): bigint {
  const derivationPath = `${CONFIG.EPSILON_DERIVATION_PREFIX},${chainId},${requester},${path}`;
  console.log('\n📋 Derivation path:', derivationPath);
  const hash = ethers.keccak256(ethers.toUtf8Bytes(derivationPath));
  console.log('📋 Epsilon hash:', hash);
  return BigInt(hash);
}

async function deriveSigningKeyWithChainId(
  path: string,
  predecessor: string,
  basePrivateKey: string,
  chainId: string
): Promise<string> {
  const epsilon = deriveEpsilonWithChainId(predecessor, path, chainId);
  const privateKeyBigInt = BigInt(basePrivateKey);
  const derivedPrivateKey = (privateKeyBigInt + epsilon) % BigInt(CONFIG.SECP256K1_N);
  return '0x' + derivedPrivateKey.toString(16).padStart(64, '0');
}

async function main() {
  console.log('='.repeat(60));
  console.log('🔍 VERIFYING ADDRESS DERIVATION FOR BTC VAULT');
  console.log('='.repeat(60));

  console.log('\n📌 Parameters:');
  console.log('   MPC Root Key:', MPC_ROOT_KEY);
  console.log('   Path:', ROOT_PATH);
  console.log('   Sender (Pallet Account):', BTC_VAULT_PALLET_ACCOUNT);
  console.log('   Chain ID:', CHAIN_ID);
  console.log('   Expected Vault Address Hash:', EXPECTED_VAULT_ADDRESS_HASH.toString('hex'));

  // Derive the private key
  const derivedPrivateKey = await deriveSigningKeyWithChainId(
    ROOT_PATH,
    BTC_VAULT_PALLET_ACCOUNT,
    MPC_ROOT_KEY,
    CHAIN_ID
  );

  console.log('\n🔑 Derived Private Key:', derivedPrivateKey);

  // Get the public key from the derived private key
  const privateKeyBuffer = Buffer.from(derivedPrivateKey.slice(2), 'hex');
  const publicKey = ecc.pointFromScalar(privateKeyBuffer, true);
  
  if (!publicKey) {
    throw new Error('Failed to derive public key');
  }

  console.log('🔑 Derived Public Key (compressed):', Buffer.from(publicKey).toString('hex'));

  // Create P2WPKH address
  const { address: bech32Address, output } = bitcoin.payments.p2wpkh({
    pubkey: Buffer.from(publicKey),
    network: bitcoin.networks.testnet,
  });

  console.log('\n📍 Derived Bitcoin Address (Bech32):', bech32Address);
  
  // The output script for P2WPKH is: OP_0 <20-byte-pubkey-hash>
  // Extract the pubkey hash (last 20 bytes after OP_0 and push byte)
  const derivedPubkeyHash = output!.slice(2); // Skip OP_0 (0x00) and push byte (0x14)
  
  console.log('📍 Derived Pubkey Hash:', derivedPubkeyHash.toString('hex'));
  console.log('📍 Expected Pubkey Hash:', EXPECTED_VAULT_ADDRESS_HASH.toString('hex'));

  // Compare
  const matches = derivedPubkeyHash.equals(EXPECTED_VAULT_ADDRESS_HASH);
  
  console.log('\n' + '='.repeat(60));
  if (matches) {
    console.log('✅ VERIFICATION PASSED: Derived address matches expected vault address!');
    console.log('   Vault Address (tb1q...):', bech32Address);
  } else {
    console.log('❌ VERIFICATION FAILED: Addresses do not match!');
    console.log('   Derived:  ', derivedPubkeyHash.toString('hex'));
    console.log('   Expected: ', EXPECTED_VAULT_ADDRESS_HASH.toString('hex'));
  }
  console.log('='.repeat(60));
}

main().catch(console.error);
