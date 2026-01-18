import { ApiPromise } from "@polkadot/api";
import { EventRecord } from "@polkadot/types/interfaces";
import { Vec } from "@polkadot/types";
import { u8aToHex } from "@polkadot/util";
import { ISubmittableResult } from "@polkadot/types/types";
import { ethers } from "ethers";
import { keccak256, recoverAddress } from "viem";

export class SignetClient {
  constructor(private api: ApiPromise, private signer: any) {}

  async ensureInitialized(chainId: string): Promise<void> {
    // Use direct RPC call to chopsticks
    const rpcCall = async (method: string, params: any[]) => {
      const response = await fetch("http://127.0.0.1:8000", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          jsonrpc: "2.0",
          id: 1,
          method,
          params,
        }),
      });
      return response.json();
    };

    // Always set storage to ensure ChainId is correct
    console.log("    Setting Signet storage via dev_setStorage...");

    // ChainId is a BoundedVec<u8> which needs SCALE encoding with length prefix
    const chainIdBytes = Buffer.from(chainId);
    const lengthPrefix = chainIdBytes.length << 2; // SCALE compact encoding for small lengths
    const scaledEncodedChainId = "0x" + Buffer.from([lengthPrefix, ...chainIdBytes]).toString("hex");

    console.log("    ChainId SCALE encoded:", scaledEncodedChainId);

    await rpcCall("dev_setStorage", [{
      Signet: {
        Admin: this.signer.address,
        SignatureDeposit: 1000000000000,
        ChainId: scaledEncodedChainId,
      }
    }]);

    // Build a new block to apply storage changes
    await rpcCall("dev_newBlock", []);

    console.log("    Signet storage set! ChainId:", chainId);
  }

  async requestSignature(payload: Uint8Array, params: any): Promise<void> {
    const tx = this.api.tx.signet.sign(
      Array.from(payload),
      params.keyVersion,
      params.path,
      params.algo,
      params.dest,
      params.params
    );

    await new Promise<void>((resolve, reject) => {
      tx.signAndSend(this.signer, (result: ISubmittableResult) => {
        const { status, dispatchError } = result;
        if (dispatchError) {
          reject(dispatchError);
        } else if (status.isInBlock) {
          resolve();
        }
      }).catch(reject);
    });
  }

  async requestTransactionSignature(
    serializedTx: number[],
    params: any
  ): Promise<void> {
    const caip2Id = params.caip2Id;
    const caip2Bytes = Array.from(new TextEncoder().encode(caip2Id));

    const tx = this.api.tx.signet.signBidirectional(
      serializedTx,
      caip2Bytes,
      params.keyVersion,
      params.path,
      params.algo || "",
      params.dest || "",
      params.params || "",
      Array.from(new TextEncoder().encode(params.schemas.explorer.schema)),
      Array.from(new TextEncoder().encode(params.schemas.callback.schema))
    );

    await tx.signAndSend(this.signer);
  }

  async waitForSignature(requestId: string, timeout: number): Promise<any> {
    return new Promise((resolve) => {
      let unsubscribe: any;
      const timer = setTimeout(() => {
        if (unsubscribe) unsubscribe();
        resolve(null);
      }, timeout);

      this.api.query.system
        .events((events: Vec<EventRecord>) => {
          events.forEach((record: EventRecord) => {
            const { event } = record;
            if (
              event.section === "signet" &&
              event.method === "SignatureResponded"
            ) {
              const [reqId, responder, signature] = event.data as any;
              if (u8aToHex(reqId.toU8a()) === requestId) {
                clearTimeout(timer);
                if (unsubscribe) unsubscribe();
                resolve({
                  responder: responder.toString(),
                  signature: signature.toJSON(),
                });
              }
            }
          });
        })
        .then((unsub: any) => {
          unsubscribe = unsub;
        });
    });
  }

  calculateRequestId(
    sender: string,
    payload: Uint8Array,
    params: any,
    chainId: string
  ): string {
    const payloadHex = "0x" + Buffer.from(payload).toString("hex");
    const encoded = ethers.AbiCoder.defaultAbiCoder().encode(
      [
        "string",
        "bytes",
        "string",
        "uint32",
        "string",
        "string",
        "string",
        "string",
      ],
      [
        sender,
        payloadHex,
        params.path,
        params.keyVersion,
        chainId,
        params.algo,
        params.dest,
        params.params,
      ]
    );
    return ethers.keccak256(encoded);
  }

  calculateSignRespondRequestId(
    sender: string,
    txData: number[],
    params: any
  ): string {
    const txHex = "0x" + Buffer.from(txData).toString("hex");
    const caip2Id = params.caip2Id;

    const encoded = ethers.solidityPacked(
      [
        "string",
        "bytes",
        "string",
        "uint32",
        "string",
        "string",
        "string",
        "string",
      ],
      [
        sender,
        txHex,
        caip2Id,
        params.keyVersion,
        params.path,
        params.algo || "",
        params.dest || "",
        params.params || "",
      ]
    );
    return ethers.keccak256(encoded);
  }

  async verifySignature(
    payload: Uint8Array,
    signature: any,
    derivedPublicKey: string
  ): Promise<boolean> {
    const r = signature.bigR.x.startsWith("0x")
      ? signature.bigR.x
      : `0x${signature.bigR.x}`;
    const s = signature.s.startsWith("0x") ? signature.s : `0x${signature.s}`;
    const v = BigInt(signature.recoveryId + 27);

    const recoveredAddress = await recoverAddress({
      hash: payload as any,
      signature: { r, s, v },
    });

    const expectedAddress =
      "0x" +
      keccak256(Buffer.from(derivedPublicKey.slice(4), "hex")).slice(-40);

    console.log("       Recovered:", recoveredAddress);
    console.log("       Expected: ", expectedAddress);

    return recoveredAddress.toLowerCase() === expectedAddress.toLowerCase();
  }

  async verifyTransactionSignature(
    tx: ethers.Transaction,
    signature: any,
    derivedPublicKey: string
  ): Promise<boolean> {
    const msgHash = ethers.keccak256(tx.unsignedSerialized);
    const r = signature.bigR.x.startsWith("0x")
      ? signature.bigR.x
      : `0x${signature.bigR.x}`;
    const s = signature.s.startsWith("0x") ? signature.s : `0x${signature.s}`;
    const v = BigInt(signature.recoveryId + 27);

    const recoveredAddress = await recoverAddress({
      hash: msgHash as `0x${string}`,
      signature: { r, s, v } as any,
    });

    const expectedAddress =
      "0x" +
      keccak256(Buffer.from(derivedPublicKey.slice(4), "hex")).slice(-40);

    console.log("       Recovered:", recoveredAddress);
    console.log("       Expected: ", expectedAddress);

    return recoveredAddress.toLowerCase() === expectedAddress.toLowerCase();
  }
}
