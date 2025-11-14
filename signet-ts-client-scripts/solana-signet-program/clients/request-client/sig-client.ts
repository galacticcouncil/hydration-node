import { AnchorProvider, Wallet } from "@coral-xyz/anchor";
import { Connection, Keypair } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import * as dotenv from "dotenv";
import * as crypto from "crypto";
import { contracts } from "signet.js";

dotenv.config({ path: path.resolve(__dirname, "../../.env") });

function loadSolanaKeypair(): Keypair {
  const keypairPath =
    process.env.KEYPAIR_PATH ||
    path.join(os.homedir(), ".config", "solana", "id.json");
  const keypairString = fs.readFileSync(keypairPath, { encoding: "utf-8" });
  const keypairData = JSON.parse(keypairString);
  return Keypair.fromSecretKey(new Uint8Array(keypairData));
}

async function main() {
  const basePublicKey = process.env.RESPONDER_BASE_PUBLIC_KEY!;
  console.log("Base public key:", basePublicKey);

  const connection = new Connection(
    process.env.RPC_URL || "https://api.devnet.solana.com",
    "confirmed"
  );

  const wallet = new Wallet(loadSolanaKeypair());
  console.log("Connected wallet address:", wallet.publicKey.toString());

  // Create provider
  const provider = new AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });

  // Create an instance of the contract class
  const contractAddress = "4uvZW8K4g4jBg7dzPNbb9XDxJLFBK7V6iC76uofmYvEU"; // Your program ID
  const contract = new contracts.solana.ChainSignatureContract({
    provider,
    programId: contractAddress,
    // rootPublicKey: basePublicKey,
  });

  // Get current deposit amount
  try {
    const depositAmount = await contract.getCurrentSignatureDeposit();
    console.log(
      "Required deposit amount:",
      depositAmount.toString(),
      "lamports"
    );
  } catch (error) {
    console.log("Error fetching deposit amount:", error);
  }

  // Generate payload and request parameters
  const path = "testPath";
  const payload = Array.from(crypto.randomBytes(32));
  const keyVersion = 0;

  console.log("Requesting signature...");

  try {
    // Use the sign method from the contract class
    const signature = await contract.sign(
      {
        payload,
        path,
        key_version: keyVersion
      },
      {
        sign: {
          algo: "",
          dest: "",
          params: "",
        },
        retry: {
          delay: 5000,
          retryCount: 12,
        },
      }
    );

    console.log("Signature successfully obtained:", signature);
    console.log("Signature components:", {
      r: signature.r,
      s: signature.s,
      v: signature.v,
    });

    // The contract.sign method already verifies the signature
    console.log("✅ Signature verified successfully!");
  } catch (error) {
    if (error instanceof contracts.solana.utils.errors.SignatureNotFoundError) {
      console.error("Signature not found:", error.message);
      console.error("Request ID:", error.requestId);
      console.error("Transaction hash:", error.hash);
    } else if (error instanceof contracts.solana.utils.errors.SignatureContractError) {
      console.error("Contract error:", error.message);
      console.error("Request ID:", error.requestId);
      console.error("Transaction hash:", error.hash);
    } else if (error instanceof contracts.solana.utils.errors.SigningError) {
      console.error("Signing error:", error.message);
      console.error("Request ID:", error.requestId);
      console.error("Transaction hash:", error.hash);
    } else {
      console.error("Unknown error:", error);
    }
    process.exit(1);
  }
}

main().catch(console.error);
