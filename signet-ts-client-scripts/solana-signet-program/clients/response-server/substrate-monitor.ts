import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import { Vec, Bytes } from "@polkadot/types";
import { EventRecord } from "@polkadot/types/interfaces";
import { u8aToHex, hexToU8a } from "@polkadot/util";
import { waitReady } from "@polkadot/wasm-crypto";

export interface SubstrateSignatureRequest {
  sender: string;
  payload: Uint8Array;
  keyVersion: number;
  deposit: string;
  chainId: string;
  path: string;
  algo: string;
  dest: string;
  params: string;
}

export interface SubstrateSignRespondRequest {
  sender: string;
  transactionData: Uint8Array;
  slip44ChainId: number;
  keyVersion: number;
  deposit: string;
  path: string;
  algo: string;
  dest: string;
  params: string;
  explorerDeserializationFormat: number;
  explorerDeserializationSchema: Uint8Array;
  callbackSerializationFormat: number;
  callbackSerializationSchema: Uint8Array;
}

export class SubstrateMonitor {
  private api: ApiPromise | null = null;
  private wsProvider: WsProvider;
  private keypair: any = null; // Add this

  constructor(private wsEndpoint: string = "ws://localhost:9944") {
    this.wsProvider = new WsProvider(wsEndpoint);
  }

  private stripScalePrefix(data: Uint8Array): Uint8Array {
    if (data.length === 0) return data;

    const mode = data[0] & 0b11; // Check last 2 bits for mode

    if (mode === 0) {
      // Single-byte compact (0-63)
      return data.slice(1);
    } else if (mode === 1) {
      // Two-byte compact (64-16383)
      return data.slice(2);
    } else if (mode === 2) {
      // Four-byte compact (16384-1073741823)
      return data.slice(4);
    }

    return data; // mode 3 = big-integer, unsupported
  }

  async connect(): Promise<void> {
    // Wait for WASM crypto to be ready
    await waitReady();

    // Custom types for your pallet
    const types = {
      SerializationFormat: {
        _enum: ["Borsh", "AbiJson"],
      },
      AffinePoint: {
        x: "[u8; 32]",
        y: "[u8; 32]",
      },
      Signature: {
        big_r: "AffinePoint",
        s: "[u8; 32]",
        recovery_id: "u8",
      },
      ErrorResponse: {
        request_id: "[u8; 32]",
        error_message: "Vec<u8>",
      },
    };

    this.api = await ApiPromise.create({
      provider: this.wsProvider,
      types,
    });

    // Initialize keypair for signing transactions
    const keyring = new Keyring({ type: "sr25519" });

    // Use environment variable or default to Alice for development
    const seed = process.env.SUBSTRATE_SIGNER_SEED || "//Alice";
    this.keypair = keyring.addFromUri(seed);

    console.log("✅ Connected to Substrate node");
    console.log("  Chain:", await this.api.rpc.system.chain());
    console.log("  Version:", await this.api.rpc.system.version());
    console.log("  Signer address:", this.keypair.address);
  }

  async sendSignatureResponse(
    requestId: Uint8Array,
    signature: any,
    signerAddress: string
  ) {
    if (!this.api) throw new Error("Not connected to Substrate");
    if (!this.keypair) throw new Error("No keypair available for signing");

    try {
      // Create the transaction
      const tx = this.api.tx.signet.respond(
        [Array.from(requestId)],
        [signature]
      );

      // Sign and send the transaction
      const hash = await tx.signAndSend(this.keypair);

      console.log("✅ Signature response sent to Substrate!");
      console.log("  Transaction hash:", hash.toHex());
      console.log("  Request ID:", u8aToHex(requestId));

      return hash;
    } catch (error) {
      console.error("❌ Failed to send signature response:", error);
      throw error;
    }
  }

  async sendReadResponse(
    requestId: Uint8Array,
    serializedOutput: Uint8Array,
    signature: any,
    signerAddress: string
  ) {
    if (!this.api) throw new Error("Not connected to Substrate");
    if (!this.keypair) throw new Error("No keypair available for signing");

    try {
      const tx = this.api.tx.signet.readRespond(
        Array.from(requestId),
        Array.from(serializedOutput),
        signature
      );

      const hash = await tx.signAndSend(this.keypair);

      console.log("✅ Read response sent to Substrate!");
      console.log("  Transaction hash:", hash.toHex());
      console.log("  Request ID:", u8aToHex(requestId));

      return hash;
    } catch (error) {
      console.error("❌ Failed to send read response:", error);
      throw error;
    }
  }

  async subscribeToEvents(handlers: {
    onSignatureRequested?: (event: SubstrateSignatureRequest) => Promise<void>;
    onSignRespondRequested?: (
      event: SubstrateSignRespondRequest
    ) => Promise<void>;
    onSignatureResponded?: (event: any) => Promise<void>;
    onReadResponded?: (event: any) => Promise<void>;
  }) {
    if (!this.api) throw new Error("Not connected to Substrate");

    // Subscribe to all events
    this.api.query.system.events((events: Vec<EventRecord>) => {
      events.forEach(async (record) => {
        const { event } = record;

        // Check if this is a Signet pallet event
        if (event.section === "signet") {
          console.log(`\n📨 Substrate Event: ${event.section}.${event.method}`);

          switch (event.method) {
            case "SignatureRequested":
              if (handlers.onSignatureRequested) {
                const [
                  sender,
                  payload,
                  keyVersion,
                  deposit,
                  chainId,
                  path,
                  algo,
                  dest,
                  params,
                ] = event.data;
                const request: SubstrateSignatureRequest = {
                  sender: sender.toString(),
                  payload: new Uint8Array((payload as any).toU8a()),
                  keyVersion: (keyVersion as any).toNumber(),
                  deposit: deposit.toString(),
                  chainId: chainId.toString(),
                  path: path.toString(),
                  algo: algo.toString(),
                  dest: dest.toString(),
                  params: params.toString(),
                };
                await handlers.onSignatureRequested(request);
              }
              break;

            case "SignRespondRequested":
              if (handlers.onSignRespondRequested) {
                const [
                  sender,
                  txData,
                  slip44ChainId,
                  keyVersion,
                  deposit,
                  path,
                  algo,
                  dest,
                  params,
                  explorerFormat,
                  explorerSchema,
                  callbackFormat,
                  callbackSchema,
                ] = event.data;

                const request: SubstrateSignRespondRequest = {
                  sender: sender.toString(),
                  transactionData: this.stripScalePrefix(new Uint8Array((txData as any).toU8a())),
                  slip44ChainId: (slip44ChainId as any).toNumber(),
                  keyVersion: (keyVersion as any).toNumber(),
                  deposit: deposit.toString(),
                  path: path.toString(),
                  algo: algo.toString(),
                  dest: dest.toString(),
                  params: params.toString(),
                  explorerDeserializationFormat: (
                    explorerFormat as any
                  ).toNumber(),
                  explorerDeserializationSchema: this.stripScalePrefix(
                    new Uint8Array((explorerSchema as any).toU8a())
                  ),
                  callbackSerializationFormat: (
                    callbackFormat as any
                  ).toNumber(),
                  callbackSerializationSchema: this.stripScalePrefix(
                    new Uint8Array((callbackSchema as any).toU8a())
                  ),
                };

                await handlers.onSignRespondRequested(request);
              }
              break;

            case "SignatureResponded":
              if (handlers.onSignatureResponded) {
                const [requestId, responder, signature] = event.data;
                await handlers.onSignatureResponded({
                  requestId: u8aToHex(
                    new Uint8Array((requestId as any).toU8a())
                  ),
                  responder: responder.toString(),
                  signature: signature.toJSON(),
                });
              }
              break;

            case "ReadResponded":
              if (handlers.onReadResponded) {
                const [requestId, responder, serializedOutput, signature] =
                  event.data;
                await handlers.onReadResponded({
                  requestId: u8aToHex(
                    new Uint8Array((requestId as any).toU8a())
                  ),
                  responder: responder.toString(),
                  serializedOutput: new Uint8Array(
                    (serializedOutput as any).toU8a()
                  ),
                  signature: signature.toJSON(),
                });
              }
              break;
          }
        }
      });
    });
  }

  async disconnect() {
    if (this.api) {
      await this.api.disconnect();
    }
  }
}
