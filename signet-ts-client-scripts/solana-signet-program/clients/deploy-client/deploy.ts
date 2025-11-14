import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import { ChainSignaturesProject } from "../../signet-program/target/types/chain_signatures_project";
import IDL from "../../signet-program/target/idl/chain_signatures_project.json";
import * as fs from "fs";
import * as path from "path";
import * as os from "os";
import * as dotenv from "dotenv";

dotenv.config({ path: path.resolve(__dirname, "../../.env") });

function loadKeypair(): Keypair {
  const keypairPath =
    process.env.KEYPAIR_PATH ||
    path.join(os.homedir(), ".config", "solana", "id.json");
  const keypairString = fs.readFileSync(keypairPath, { encoding: "utf-8" });
  const keypairData = JSON.parse(keypairString);
  return Keypair.fromSecretKey(new Uint8Array(keypairData));
}

const SIGNATURE_DEPOSIT = new anchor.BN(0.01 * LAMPORTS_PER_SOL); // 0.01 SOL

async function main() {
  const connection = new Connection(
    process.env.RPC_URL || "https://api.devnet.solana.com",
    "confirmed"
  );
  const wallet = new anchor.Wallet(loadKeypair());
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);

  const program = new Program<ChainSignaturesProject>(IDL, provider);

  console.log("Using wallet:", wallet.publicKey.toString());

  const [programStatePDA] = PublicKey.findProgramAddressSync(
    [Buffer.from("program-state")],
    program.programId
  );

  console.log("Program State PDA:", programStatePDA.toString());

  try {
    console.log("Initializing program...");
    const tx = await program.methods
      .initialize(SIGNATURE_DEPOSIT)
      .accounts({
        admin: wallet.publicKey,
      })
      .rpc();

    console.log("Program initialized successfully!");
    console.log("Transaction signature:", tx);
    console.log(
      "Signature deposit set to:",
      SIGNATURE_DEPOSIT.toString(),
      "lamports"
    );
  } catch (error) {
    console.error("Error initializing program:", error);
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
