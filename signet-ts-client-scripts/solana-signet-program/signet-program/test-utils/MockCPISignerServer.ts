import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ChainSignaturesProject } from "../target/types/chain_signatures_project";
import { contracts } from "signet.js";
import { getEnv, deriveSigningKey, signMessage } from "./utils";
import {
  ANCHOR_EMIT_CPI_CALL_BACK_DISCRIMINATOR,
  eventNames,
} from "./constants";
import { SignatureRequestedEvent } from "./types";

const env = getEnv();

export async function parseCPIEvents(
  connection: anchor.web3.Connection,
  signature: string,
  targetProgramId: anchor.web3.PublicKey,
  program: Program<ChainSignaturesProject>
): Promise<SignatureRequestedEvent[]> {
  const tx = await connection.getTransaction(signature, {
    commitment: "confirmed",
    maxSupportedTransactionVersion: 0,
  });

  if (!tx?.meta?.innerInstructions) {
    return [];
  }

  const targetProgramStr = targetProgramId.toString();
  const events: SignatureRequestedEvent[] = [];

  // Get account keys properly based on transaction type
  const getAccountKeys = (): anchor.web3.PublicKey[] => {
    const message = tx.transaction.message;
    if ("accountKeys" in message) {
      // Legacy transaction
      return message.accountKeys;
    } else {
      // Versioned transaction
      return message.getAccountKeys().staticAccountKeys;
    }
  };

  const accountKeys = getAccountKeys();

  for (const innerIxSet of tx.meta.innerInstructions) {
    for (const instruction of innerIxSet.instructions) {
      if (!instruction.data || instruction.programIdIndex >= accountKeys.length)
        continue;

      const programKey = accountKeys[instruction.programIdIndex];

      if (programKey.toString() === targetProgramStr) {
        try {
          const rawData = anchor.utils.bytes.bs58.decode(instruction.data);

          if (
            !rawData
              .subarray(0, 8)
              .equals(ANCHOR_EMIT_CPI_CALL_BACK_DISCRIMINATOR)
          ) {
            continue;
          }

          const eventData = anchor.utils.bytes.base64.encode(
            rawData.subarray(8)
          );
          const event = program.coder.events.decode(eventData);

          if (event?.name === eventNames.signatureRequested) {
            events.push(event.data as SignatureRequestedEvent);
          }
        } catch {
          // Ignore non-event instructions
        }
      }
    }
  }

  return events;
}

export class MockCPISignerServer {
  private readonly program: Program<ChainSignaturesProject>;
  private readonly solContract: contracts.solana.ChainSignatureContract;
  private readonly wallet: anchor.Wallet;
  private readonly provider: anchor.AnchorProvider;
  private readonly signetProgramId: anchor.web3.PublicKey;
  private logSubscriptionId: number | null = null;

  constructor({
    provider,
    signetSolContract,
    signetProgramId,
  }: {
    provider: anchor.AnchorProvider;
    signetSolContract: contracts.solana.ChainSignatureContract;
    signetProgramId: anchor.web3.PublicKey;
  }) {
    this.provider = provider;
    this.wallet = provider.wallet as anchor.Wallet;
    this.program = anchor.workspace
      .chainSignaturesProject as Program<ChainSignaturesProject>;
    this.solContract = signetSolContract;
    this.signetProgramId = signetProgramId;
  }

  async start(): Promise<void> {
    await this.subscribeToEvents();
  }

  async stop(): Promise<void> {
    if (this.logSubscriptionId !== null) {
      await this.provider.connection.removeOnLogsListener(
        this.logSubscriptionId
      );
      this.logSubscriptionId = null;
    }
  }

  private async subscribeToEvents(): Promise<void> {
    this.logSubscriptionId = this.provider.connection.onLogs(
      this.signetProgramId,
      async (logs) => {
        try {
          const events = await parseCPIEvents(
            this.provider.connection,
            logs.signature,
            this.signetProgramId,
            this.program
          );

          await Promise.all(
            events.map((event) => this.handleSignatureRequest(event))
          );
        } catch (error) {
          console.error("Error processing CPI event:", error);
        }
      },
      "confirmed"
    );
  }

  private async handleSignatureRequest(
    eventData: SignatureRequestedEvent
  ): Promise<void> {
    try {
      const requestId = this.solContract.getRequestId(
        {
          payload: eventData.payload,
          path: eventData.path,
          key_version: eventData.keyVersion,
        },
        {
          algo: eventData.algo,
          dest: eventData.dest,
          params: eventData.params,
        }
      );

      const requestIdBytes = Array.from(Buffer.from(requestId.slice(2), "hex"));

      const derivedPrivateKey = await deriveSigningKey(
        eventData.path,
        eventData.sender.toString(),
        env.PRIVATE_KEY_TESTNET
      );

      const signature = await signMessage(eventData.payload, derivedPrivateKey);

      await this.program.methods
        .respond([requestIdBytes], [signature])
        .accounts({ responder: this.wallet.publicKey })
        .rpc();
    } catch (error) {
      console.error("Error sending signature response:", error);
    }
  }
}
