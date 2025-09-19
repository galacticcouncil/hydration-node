import { ethers } from "ethers";
import { ec as EC } from "elliptic";

export class KeyDerivation {
  private static readonly EPSILON_PREFIX = "sig.network v1.0.0 epsilon derivation";

  static derivePublicKey(
    rootPublicKey: string,
    predecessorId: string,
    path: string,
    chainId: string
  ): string {
    const ec = new EC("secp256k1");
    const uncompressedRoot = rootPublicKey.slice(2);
    
    const derivationPath = `${this.EPSILON_PREFIX},${chainId},${predecessorId},${path}`;
    const hash = ethers.keccak256(ethers.toUtf8Bytes(derivationPath));
    const scalarHex = hash.slice(2);

    const x = uncompressedRoot.substring(2, 66);
    const y = uncompressedRoot.substring(66);
    const oldPoint = ec.curve.point(x, y);
    const scalarTimesG = ec.g.mul(scalarHex);
    const newPoint = oldPoint.add(scalarTimesG);
    
    return `04${newPoint.getX().toString("hex").padStart(64, "0")}${newPoint.getY().toString("hex").padStart(64, "0")}`;
  }
}