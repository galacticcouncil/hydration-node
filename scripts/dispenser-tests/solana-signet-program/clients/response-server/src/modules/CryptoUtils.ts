import { CONFIG } from '../config/Config';
import { ethers } from 'ethers';
import { SignatureResponse } from '../types';
import * as ecc from 'tiny-secp256k1';

export class CryptoUtils {
  static deriveEpsilon(requester: string, path: string): bigint {
    const derivationPath = `${CONFIG.EPSILON_DERIVATION_PREFIX},${CONFIG.SOLANA_CHAIN_ID},${requester},${path}`;
    const hash = ethers.keccak256(ethers.toUtf8Bytes(derivationPath));
    return BigInt(hash);
  }

  static deriveEpsilonWithChainId(
    requester: string,
    path: string,
    chainId: string
  ): bigint {
    const derivationPath = `${CONFIG.EPSILON_DERIVATION_PREFIX},${chainId},${requester},${path}`;
    const hash = ethers.keccak256(ethers.toUtf8Bytes(derivationPath));
    return BigInt(hash);
  }

  static async deriveSigningKey(
    path: string,
    predecessor: string,
    basePrivateKey: string
  ): Promise<string> {
    const epsilon = this.deriveEpsilon(predecessor, path);
    const privateKeyBigInt = BigInt(basePrivateKey);
    const derivedPrivateKey =
      (privateKeyBigInt + epsilon) % BigInt(CONFIG.SECP256K1_N);
    return '0x' + derivedPrivateKey.toString(16).padStart(64, '0');
  }

  static async deriveSigningKeyWithChainId(
    path: string,
    predecessor: string,
    basePrivateKey: string,
    chainId: string
  ): Promise<string> {
    const epsilon = this.deriveEpsilonWithChainId(predecessor, path, chainId);
    const privateKeyBigInt = BigInt(basePrivateKey);
    const derivedPrivateKey =
      (privateKeyBigInt + epsilon) % BigInt(CONFIG.SECP256K1_N);
    return '0x' + derivedPrivateKey.toString(16).padStart(64, '0');
  }

  static async signMessage(
    msgHash: number[] | string,
    privateKeyHex: string
  ): Promise<SignatureResponse> {
    const msgHashHex =
      typeof msgHash === 'string'
        ? msgHash
        : '0x' + Buffer.from(msgHash).toString('hex');

    console.log('🔍 signMessage received:', msgHashHex);

    const signingKey = new ethers.SigningKey(privateKeyHex);
    const signature = signingKey.sign(msgHashHex);

    console.log('🔍 signMessage produced r:', signature.r);
    console.log('🔍 signMessage produced s:', signature.s);

    const recoveredPublicKey = ethers.SigningKey.recoverPublicKey(
      msgHashHex,
      signature
    );
    const publicKeyPoint = ethers.getBytes(recoveredPublicKey);

    const y = publicKeyPoint.slice(33, 65);

    return {
      bigR: {
        x: Array.from(Buffer.from(signature.r.slice(2), 'hex')),
        y: Array.from(y),
      },
      s: Array.from(Buffer.from(signature.s.slice(2), 'hex')),
      recoveryId: signature.v - 27,
    };
  }

  static async signBidirectionalResponse(
    requestId: Uint8Array,
    serializedOutput: Uint8Array,
    privateKeyHex: string
  ): Promise<SignatureResponse> {
    const combined = new Uint8Array(requestId.length + serializedOutput.length);
    combined.set(requestId);
    combined.set(serializedOutput, requestId.length);
    const messageHash = ethers.keccak256(combined);
    return this.signMessage(messageHash, privateKeyHex);
  }

  static async signDigestDirectly(
    digest: Uint8Array,
    privateKeyHex: string
  ): Promise<SignatureResponse> {
    const privateKeyBytes = Buffer.from(privateKeyHex.slice(2), 'hex');

    // Use ecc.sign instead of ecc.signRecoverable (same as bitcoinjs-lib internally)
    const signature = ecc.sign(digest, privateKeyBytes);

    if (!signature) {
      throw new Error('Failed to sign digest');
    }

    let r = signature.slice(0, 32);
    let s = signature.slice(32, 64);

    // Bitcoin requires low-s (BIP 62)
    const N = BigInt(
      '0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141'
    );
    const halfN = N / 2n;
    const sBigInt = BigInt('0x' + Buffer.from(s).toString('hex'));

    let recovery = 0;
    if (sBigInt > halfN) {
      const normalizedS = N - sBigInt;
      s = Buffer.from(normalizedS.toString(16).padStart(64, '0'), 'hex');
      recovery = 1;
    }

    // Get public key for bigR.y
    const publicKey = ecc.pointFromScalar(privateKeyBytes, false);
    if (!publicKey) {
      throw new Error('Failed to derive public key');
    }
    const y = publicKey.slice(33, 65);

    return {
      bigR: {
        x: Array.from(r),
        y: Array.from(y),
      },
      s: Array.from(s),
      recoveryId: recovery,
    };
  }
}
