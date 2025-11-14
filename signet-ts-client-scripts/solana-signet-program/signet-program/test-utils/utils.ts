import * as path from "path";
import * as dotenv from "dotenv";
import { z } from "zod";
import { ethers } from "ethers";
import bs58 from "bs58";
import {
  EPSILON_DERIVATION_PREFIX,
  SOLANA_CHAIN_ID,
  SECP256K1_CURVE_ORDER,
} from "./constants";
import * as anchor from "@coral-xyz/anchor";

// Load environment variables from the root .env file
dotenv.config({ path: path.resolve(__dirname, "../../.env") });

const envSchema = z.object({
  PRIVATE_KEY_TESTNET: z.string().min(1, "PRIVATE_KEY_TESTNET is required"),
});

export const getEnv = () => {
  const result = envSchema.safeParse({
    PRIVATE_KEY_TESTNET: process.env.PRIVATE_KEY_TESTNET,
  });

  if (!result.success) {
    throw new Error(`Environment validation failed: ${result.error.message}`);
  }

  return result.data;
};

/**
 * Converts a private key to a NEAR Account JSON (NAJ) formatted public key.
 *
 * @param privateKey - The private key in hex format (with or without 0x prefix)
 * @returns A NEAR-formatted public key string in the format `secp256k1:{base58_encoded_coordinates}`
 *
 * @example
 * ```typescript
 * const privateKey = "0x1234567890abcdef...";
 * const najPublicKey = bigintPrivateKeyToNajPublicKey(privateKey);
 * // Returns: "secp256k1:ABC123..." where ABC123... is the base58-encoded x,y coordinates
 * ```
 */
export const bigintPrivateKeyToNajPublicKey = (
  privateKey: string
): `secp256k1:${string}` => {
  const signingKey = new ethers.SigningKey(privateKey);
  const publicKeyPoint = signingKey.publicKey;

  const publicKeyHex = publicKeyPoint.slice(4); // Remove '0x04' prefix
  const xCoord = publicKeyHex.slice(0, 64); // First 32 bytes (64 hex chars)
  const yCoord = publicKeyHex.slice(64, 128); // Second 32 bytes (64 hex chars)

  const xBytes = Buffer.from(xCoord, "hex");
  const yBytes = Buffer.from(yCoord, "hex");
  const publicKeyBytes = Buffer.concat([xBytes, yBytes]);

  const publicKeyBase58 = bs58.encode(publicKeyBytes);

  return `secp256k1:${publicKeyBase58}`;
};

export interface SignatureResult {
  bigR: {
    x: number[];
    y: number[];
  };
  s: number[];
  recoveryId: number;
}
export async function deriveSigningKey(
  path: string,
  predecessor: string,
  basePrivateKey: string
): Promise<string> {
  const derivationPath = `${EPSILON_DERIVATION_PREFIX},${SOLANA_CHAIN_ID},${predecessor},${path}`;
  const epsilonHash = ethers.keccak256(ethers.toUtf8Bytes(derivationPath));
  const epsilon = BigInt(epsilonHash);

  const basePrivateKeyBigInt = BigInt(basePrivateKey);
  const derivedPrivateKey =
    (basePrivateKeyBigInt + epsilon) % SECP256K1_CURVE_ORDER;

  return "0x" + derivedPrivateKey.toString(16).padStart(64, "0");
}

export async function signMessage(
  msgHash: number[] | string,
  privateKeyHex: string
): Promise<SignatureResult> {
  const msgHashHex =
    typeof msgHash === "string"
      ? msgHash
      : "0x" + Buffer.from(msgHash).toString("hex");

  const signingKey = new ethers.SigningKey(privateKeyHex);
  const signature = signingKey.sign(msgHashHex);

  const recoveredPublicKey = ethers.SigningKey.recoverPublicKey(
    msgHashHex,
    signature
  );
  const publicKeyPoint = ethers.getBytes(recoveredPublicKey);

  const x = publicKeyPoint.slice(1, 33);
  const y = publicKeyPoint.slice(33, 65);

  return {
    bigR: {
      x: Array.from(Buffer.from(signature.r.slice(2), "hex")),
      y: Array.from(y),
    },
    s: Array.from(Buffer.from(signature.s.slice(2), "hex")),
    recoveryId: signature.v - 27,
  };
}

export const confirmTransaction = async (
  connection: anchor.web3.Connection,
  signature: string
) => {
  const latestBlockhash = await connection.getLatestBlockhash();

  await connection.confirmTransaction(
    {
      signature,
      blockhash: latestBlockhash.blockhash,
      lastValidBlockHeight: latestBlockhash.lastValidBlockHeight,
    },
    "confirmed"
  );
};
