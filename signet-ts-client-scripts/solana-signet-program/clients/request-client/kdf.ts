import { ethers } from "ethers";
import { secp256k1 } from "@noble/curves/secp256k1";

const EPSILON_DERIVATION_PREFIX = "sig.network v1.0.0 epsilon derivation";

/**
 * Derive epsilon value for key derivation
 * @param requester The address of the requester
 * @param path The derivation path
 * @returns A bigint representing the derived epsilon value
 */
export function deriveEpsilonSol(requester: string, path: string): bigint {
  // Solana SLIP-0044 https://github.com/satoshilabs/slips/blob/master/slip-0044.md
  const chainId = "0x800001f5";

  const derivationPath = `${EPSILON_DERIVATION_PREFIX},${chainId},${requester},${path}`;
  console.log("Derivation path:", derivationPath);

  const hash = ethers.keccak256(ethers.toUtf8Bytes(derivationPath));
  return BigInt(hash);
}

/**
 * Convert a public key string to x,y point coordinates
 * @param publicKey The public key in string format (0x04 + x + y)
 * @returns An object with x and y coordinates as bigints
 */
export function publicKeyToPoint(publicKey: string) {
  // Remove '0x04' prefix
  const cleanPubKey = publicKey.slice(4);
  const x = cleanPubKey.slice(0, 64);
  const y = cleanPubKey.slice(64, 128);
  return { x: BigInt("0x" + x), y: BigInt("0x" + y) };
}

/**
 * Convert x,y point coordinates to a public key string
 * @param point Object with x and y coordinates as bigints
 * @returns The public key in string format (0x04 + x + y)
 */
export function pointToPublicKey(point: { x: bigint; y: bigint }): string {
  const x = point.x.toString(16).padStart(64, "0");
  const y = point.y.toString(16).padStart(64, "0");
  return "0x04" + x + y;
}

/**
 * Derive a public key using Ethereum-compatible derivation
 * @param path The derivation path
 * @param requesterAddress The address of the requester
 * @param basePublicKey The base public key
 * @returns The derived public key
 */
export function derivePublicKey(
  path: string,
  requesterAddress: string,
  basePublicKey: string
): string {
  try {
    const epsilon = deriveEpsilonSol(requesterAddress, path);
    const basePoint = publicKeyToPoint(basePublicKey);

    const epsilonPoint = secp256k1.ProjectivePoint.BASE.multiply(epsilon);
    const baseProjectivePoint = new secp256k1.ProjectivePoint(
      basePoint.x,
      basePoint.y,
      1n
    );

    // Add the points: (G * ε) + basePublicKey
    const resultPoint = epsilonPoint.add(baseProjectivePoint);
    const resultAffine = resultPoint.toAffine();

    const derivedPublicKey = pointToPublicKey({
      x: resultAffine.x,
      y: resultAffine.y,
    });

    console.log("Derived public key:", derivedPublicKey);
    return derivedPublicKey;
  } catch (error) {
    console.error("Error deriving public key:", error);
    throw error;
  }
}
