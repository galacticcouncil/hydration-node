import { Program } from "@coral-xyz/anchor";
import { ChainSignaturesProject } from "../target/types/chain_signatures_project";
import { ethers } from "ethers";
import { eventNames } from "./constants";

interface SignatureResponse {
  isValid: boolean;
  recoveredAddress?: string;
  derivedAddress?: string;
  error?: string;
}

export class SignatureRespondedSubscriber {
  private program: Program<ChainSignaturesProject>;

  constructor(program: Program<ChainSignaturesProject>) {
    this.program = program;
  }

  async waitForSignatureResponse({
    requestId,
    expectedPayload,
    expectedDerivedAddress,
    timeoutMs = 60000,
  }: {
    requestId: string;
    expectedPayload: Buffer;
    expectedDerivedAddress: string;
    timeoutMs?: number;
  }): Promise<SignatureResponse> {
    return new Promise((resolve, reject) => {
      let listener: number;
      let timeoutId: NodeJS.Timeout;

      const cleanup = async () => {
        if (timeoutId) clearTimeout(timeoutId);

        if (listener !== undefined) {
          await this.program.removeEventListener(listener);
        }
      };

      listener = this.program.addEventListener(
        eventNames.signatureResponded,
        async (event) => {
          try {
            const eventRequestIdHex =
              "0x" + Buffer.from(event.requestId).toString("hex");

            if (eventRequestIdHex !== requestId) {
              return;
            }

            const signature = event.signature;
            const bigRx = "0x" + Buffer.from(signature.bigR.x).toString("hex");
            const s = "0x" + Buffer.from(signature.s).toString("hex");
            const recoveryId = signature.recoveryId;

            const sig = {
              r: bigRx,
              s,
              v: recoveryId + 27,
            };

            const payloadHex = "0x" + expectedPayload.toString("hex");
            const recoveredAddress = ethers.recoverAddress(payloadHex, sig);

            const isValid =
              recoveredAddress.toLowerCase() ===
              expectedDerivedAddress.toLowerCase();

            await cleanup();
            resolve({
              isValid,
              recoveredAddress,
              derivedAddress: expectedDerivedAddress,
            });
          } catch (error: any) {
            await cleanup();
            resolve({
              isValid: false,
              error: error.message,
            });
          }
        }
      );

      timeoutId = setTimeout(async () => {
        await cleanup();
        reject(new Error("Timeout waiting for signature response"));
      }, timeoutMs);
    });
  }
}
