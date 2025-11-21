import { CONFIG } from "./config";
import { ethers } from "ethers";

export class CryptoUtils {
  static deriveEpsilon(requester: string, path: string): bigint {
    const derivationPath = `${CONFIG.EPSILON_DERIVATION_PREFIX},${CONFIG.SOLANA_CHAIN_ID},${requester},${path}`;
    console.log("📝 Derivation path:", derivationPath);
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
    return "0x" + derivedPrivateKey.toString(16).padStart(64, "0");
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
    return "0x" + derivedPrivateKey.toString(16).padStart(64, "0");
  }

  static deriveEpsilonWithChainId(
    requester: string,
    path: string,
    chainId: string
  ): bigint {
    const derivationPath = `${CONFIG.EPSILON_DERIVATION_PREFIX},${chainId},${requester},${path}`;
    console.log("📝 Derivation path:", derivationPath);
    const hash = ethers.keccak256(ethers.toUtf8Bytes(derivationPath));
    return BigInt(hash);
  }

  static modularSquareRoot(n: bigint, p: bigint): bigint {
    if (n === 0n) return 0n;
    if (p % 4n === 3n) {
      return this.powerMod(n, (p + 1n) / 4n, p);
    }
    throw new Error("Modulus not supported");
  }

  static powerMod(base: bigint, exponent: bigint, modulus: bigint): bigint {
    if (modulus === 1n) return 0n;
    let result = 1n;
    base = base % modulus;
    while (exponent > 0n) {
      if (exponent % 2n === 1n) {
        result = (result * base) % modulus;
      }
      base = (base * base) % modulus;
      exponent = exponent / 2n;
    }
    return result;
  }

  static async signMessage(
    msgHash: number[] | string,
    privateKeyHex: string
  ): Promise<any> {
    const msgHashHex =
      typeof msgHash === "string"
        ? msgHash
        : "0x" + Buffer.from(msgHash).toString("hex");

    const signingKey = new ethers.SigningKey(privateKeyHex);
    const signature = signingKey.sign(msgHashHex);

    // Convert to Solana format
    const rBigInt = BigInt(signature.r);
    const p = BigInt(CONFIG.SECP256K1_P);
    const ySquared = (rBigInt ** 3n + 7n) % p;
    const y = this.modularSquareRoot(ySquared, p);
    const recoveryId = signature.v - 27;
    const yParity = recoveryId;
    const rY = y % 2n === BigInt(yParity) ? y : p - y;

    return {
      bigR: {
        x: Array.from(Buffer.from(signature.r.slice(2), "hex")),
        y: Array.from(Buffer.from(rY.toString(16).padStart(64, "0"), "hex")),
      },
      s: Array.from(Buffer.from(signature.s.slice(2), "hex")),
      recoveryId,
    };
  }

  static hashMessage(
    requestId: Uint8Array,
    serializedOutput: Uint8Array
  ): string {
    const combined = new Uint8Array(requestId.length + serializedOutput.length);
    combined.set(requestId);
    combined.set(serializedOutput, requestId.length);
    return ethers.keccak256(combined);
  }
}
