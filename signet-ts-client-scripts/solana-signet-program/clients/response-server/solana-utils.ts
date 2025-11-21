import { Keypair } from "@solana/web3.js";
import path from "path";
import * as fs from "fs";
import * as os from "os";

export class SolanaUtils {
  static loadKeypair(): Keypair {
    if (process.env.SOLANA_PRIVATE_KEY) {
      try {
        const privateKey = JSON.parse(process.env.SOLANA_PRIVATE_KEY);
        return Keypair.fromSecretKey(new Uint8Array(privateKey));
      } catch (e) {
        throw new Error(`Failed to parse SOLANA_PRIVATE_KEY: ${e}`);
      }
    }

    try {
      const keypairPath =
        process.env.KEYPAIR_PATH ||
        path.join(os.homedir(), ".config", "solana", "id.json");
      const keypairString = fs.readFileSync(keypairPath, { encoding: "utf-8" });
      const keypairData = JSON.parse(keypairString);
      return Keypair.fromSecretKey(new Uint8Array(keypairData));
    } catch (e) {
      throw new Error(`Failed to load keypair from file: ${e}`);
    }
  }
}
