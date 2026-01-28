import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
import { Vec, Bytes } from '@polkadot/types';
import { EventRecord } from '@polkadot/types/interfaces';
import { u8aToHex, hexToU8a } from '@polkadot/util';
import { waitReady } from '@polkadot/wasm-crypto';

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

export interface SubstrateBidirectionalRequest {
  sender: string;
  serializedTransaction: Uint8Array;
  caip2Id: string;
  keyVersion: number;
  deposit: string;
  path: string;
  algo: string;
  dest: string;
  params: string;
  outputDeserializationSchema: Uint8Array;
  respondSerializationSchema: Uint8Array;
}

export class SubstrateMonitor {
  private api: ApiPromise | null = null;
  private wsProvider: WsProvider;
  private keypair: any = null;
  private txQueue: Promise<any> = Promise.resolve(); // Transaction queue to prevent nonce conflicts

  constructor(private wsEndpoint: string = 'ws://localhost:9944') {
    this.wsProvider = new WsProvider(wsEndpoint);
  }

  /**
   * Queue a transaction to ensure sequential execution and prevent nonce conflicts.
   * Each transaction waits for the previous one to complete before starting.
   */
  private async queueTransaction<T>(fn: () => Promise<T>): Promise<T> {
    const previousTx = this.txQueue;
    let resolve: (value: T) => void;
    let reject: (error: any) => void;

    const thisResult = new Promise<T>((res, rej) => {
      resolve = res;
      reject = rej;
    });

    this.txQueue = thisResult.catch(() => {}); // Don't let failures block the queue

    try {
      await previousTx; // Wait for previous transaction to complete
    } catch {
      // Previous transaction failed, but we can still proceed
    }

    try {
      const result = await fn();
      resolve!(result);
      return result;
    } catch (error) {
      reject!(error);
      throw error;
    }
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
        _enum: ['Borsh', 'AbiJson'],
      },
      AffinePoint: {
        x: '[u8; 32]',
        y: '[u8; 32]',
      },
      Signature: {
        big_r: 'AffinePoint',
        s: '[u8; 32]',
        recovery_id: 'u8',
      },
      ErrorResponse: {
        request_id: '[u8; 32]',
        error_message: 'Vec<u8>',
      },
    };

    this.api = await ApiPromise.create({
      provider: this.wsProvider,
      types,
    });

    // Initialize keypair for signing transactions
    const keyring = new Keyring({ type: 'sr25519' });

    // Use environment variable or default to Alice for development
    const seed = process.env.SUBSTRATE_SIGNER_SEED || '//Alice';
    this.keypair = keyring.addFromUri(seed);

    console.log('✅ Connected to Substrate node');
    console.log('  Chain:', await this.api.rpc.system.chain());
    console.log('  Version:', await this.api.rpc.system.version());
    console.log('  Signer address:', this.keypair.address);
  }

  async sendSignatureResponse(
    requestId: Uint8Array,
    signature: any
  ) {
    if (!this.api) throw new Error('Not connected to Substrate');
    if (!this.keypair) throw new Error('No keypair available for signing');

    // Queue this transaction to prevent nonce conflicts
    return this.queueTransaction(async () => {
      try {
        // Create the transaction
        const tx = this.api!.tx.signet.respond(
          [Array.from(requestId)],
          [signature]
        );

        // Wait for the transaction to be included in a block
        return new Promise((resolve, reject) => {
          tx.signAndSend(this.keypair!, (result: any) => {
            const { status, dispatchError } = result;
            if (status.isInBlock) {
              console.log('✅ Signature response sent to Substrate!');
              console.log('  Transaction hash:', status.asInBlock.toHex());
              console.log('  Request ID:', u8aToHex(requestId));

              if (dispatchError) {
                if (dispatchError.isModule && this.api) {
                  const decoded = this.api.registry.findMetaError(dispatchError.asModule);
                  console.error(`❌ Dispatch error: ${decoded.section}.${decoded.name}`);
                  reject(new Error(`${decoded.section}.${decoded.name}`));
                } else {
                  console.error('❌ Dispatch error:', dispatchError.toString());
                  reject(new Error(dispatchError.toString()));
                }
                return;
              }

              resolve(status.asInBlock);
            } else if (status.isDropped || status.isInvalid || status.isUsurped) {
              console.error(`❌ Signature response failed with status: ${status.type}`);
              reject(new Error(`Transaction ${status.type}`));
            }
          }).catch(reject);
        });
      } catch (error) {
        console.error('❌ Failed to send signature response:', error);
        throw error;
      }
    });
  }

  async sendRespondBidirectional(
    requestId: Uint8Array,
    serializedOutput: Uint8Array,
    signature: any,
    maxRetries: number = 3
  ) {
    if (!this.api) throw new Error('Not connected to Substrate');
    if (!this.keypair) throw new Error('No keypair available for signing');

    // Queue this transaction to prevent nonce conflicts
    return this.queueTransaction(async () => {
      let attempt = 0;

      while (attempt <= maxRetries) {
        try {
          const tx = this.api!.tx.signet.respondBidirectional(
            Array.from(requestId),
            Array.from(serializedOutput),
            signature
          );

          // Wait for the transaction to be included in a block (not just submitted)
          return await new Promise((resolve, reject) => {
            tx.signAndSend(this.keypair!, (result: any) => {
              const { status, dispatchError, events } = result;
              if (status.isInBlock) {
                console.log('✅ Bidirectional response sent to Substrate!');
                console.log('  Transaction hash:', status.asInBlock.toHex());
                console.log('  Request ID:', u8aToHex(requestId));

                if (dispatchError) {
                  if (dispatchError.isModule && this.api) {
                    const decoded = this.api.registry.findMetaError(dispatchError.asModule);
                    console.error(`❌ Dispatch error: ${decoded.section}.${decoded.name}: ${decoded.docs.join(' ')}`);
                    reject(new Error(`${decoded.section}.${decoded.name}`));
                  } else {
                    console.error('❌ Dispatch error:', dispatchError.toString());
                    reject(new Error(dispatchError.toString()));
                  }
                  return;
                }

                // Log emitted events
                events.forEach(({ event }: any) => {
                  if (event.section === 'signet') {
                    console.log(`  📨 Event: signet.${event.method}`);
                  }
                });

                resolve(status.asInBlock);
              } else if (status.isDropped || status.isInvalid || status.isUsurped) {
                console.error(`❌ Transaction failed with status: ${status.type}`);
                reject(new Error(`Transaction ${status.type}`));
              }
            }).catch(reject);
          });
        } catch (error: any) {
          const errorStr = error?.message || String(error);
          // Retry on stale nonce errors
          if (errorStr.includes('stale') && attempt < maxRetries) {
            attempt++;
            console.log(`⚠️ Stale nonce detected, retrying (attempt ${attempt}/${maxRetries})...`);
            await new Promise(r => setTimeout(r, 2000)); // Wait 2 seconds before retry
            continue;
          }
          console.error('❌ Failed to send bidirectional response:', error);
          throw error;
        }
      }
      throw new Error('Max retries exceeded for sendRespondBidirectional');
    });
  }

  async subscribeToEvents(handlers: {
    onSignatureRequested?: (event: SubstrateSignatureRequest) => Promise<void>;
    onSignBidirectional?: (
      event: SubstrateBidirectionalRequest
    ) => Promise<void>;
    onSignatureResponded?: (event: any) => Promise<void>;
    onRespondBidirectional?: (event: any) => Promise<void>;
  }) {
    if (!this.api) throw new Error('Not connected to Substrate');

    // Subscribe to all events
    this.api.query.system.events((events: Vec<EventRecord>) => {
      events.forEach(async (record: EventRecord) => {
        const { event } = record;

        // Check if this is a Signet pallet event
        if (event.section === 'signet') {
          console.log(`\n📨 Substrate Event: ${event.section}.${event.method}`);

          switch (event.method) {
            case 'SignatureRequested':
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

            case 'SignBidirectionalRequested':
              if (handlers.onSignBidirectional) {
                const [
                  sender,
                  serializedTx,
                  caip2Id,
                  keyVersion,
                  deposit,
                  path,
                  algo,
                  dest,
                  params,
                  outputSchema,
                  respondSchema,
                ] = event.data;

                const request: SubstrateBidirectionalRequest = {
                  sender: sender.toString(),
                  serializedTransaction: this.stripScalePrefix(
                    new Uint8Array((serializedTx as any).toU8a())
                  ),
                  caip2Id: new TextDecoder().decode(
                    this.stripScalePrefix(
                      new Uint8Array((caip2Id as any).toU8a())
                    )
                  ),
                  keyVersion: (keyVersion as any).toNumber(),
                  deposit: deposit.toString(),
                  path: path.toString(),
                  algo: algo.toString(),
                  dest: dest.toString(),
                  params: params.toString(),
                  outputDeserializationSchema: this.stripScalePrefix(
                    new Uint8Array((outputSchema as any).toU8a())
                  ),
                  respondSerializationSchema: this.stripScalePrefix(
                    new Uint8Array((respondSchema as any).toU8a())
                  ),
                };

                await handlers.onSignBidirectional(request);
              }
              break;

            case 'SignatureResponded':
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

            case 'RespondBidirectionalEvent':
              if (handlers.onRespondBidirectional) {
                const [requestId, responder, serializedOutput, signature] =
                  event.data;
                await handlers.onRespondBidirectional({
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
