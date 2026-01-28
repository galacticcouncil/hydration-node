import {
  IBitcoinAdapter,
  BitcoinTransactionInfo,
  UTXO,
} from './IBitcoinAdapter';
import type { BitcoinNetwork } from '../types/index.js';

interface MempoolTransaction {
  txid: string;
  status: {
    confirmed: boolean;
    block_height?: number;
    block_hash?: string;
  };
}

/**
 * Mempool.space API adapter for Bitcoin testnet
 *
 * Public API (no auth required, ~1 req/sec rate limit)
 * Does NOT support regtest - use BitcoinCoreRpcAdapter instead
 */
export class MempoolSpaceAdapter implements IBitcoinAdapter {
  private baseUrl: string;

  private constructor(baseUrl: string) {
    this.baseUrl = baseUrl;
  }

  static create(network: BitcoinNetwork): MempoolSpaceAdapter {
    if (network !== 'testnet') {
      throw new Error(
        `Unsupported mempool.space network '${network}'. Only testnet requests use this adapter.`
      );
    }

    return new MempoolSpaceAdapter('https://mempool.space/testnet4/api');
  }

  async isAvailable(): Promise<boolean> {
    try {
      const response = await fetch(`${this.baseUrl}/blocks/tip/height`, {
        signal: AbortSignal.timeout(5000),
      });
      return response.ok;
    } catch {
      return false;
    }
  }

  async getTransaction(txid: string): Promise<BitcoinTransactionInfo> {
    const response = await fetch(`${this.baseUrl}/tx/${txid}`);

    if (!response.ok) {
      throw new Error(`Transaction ${txid} not found`);
    }

    const tx = (await response.json()) as MempoolTransaction;
    const currentHeight = await this.getCurrentBlockHeight();

    const confirmations =
      tx.status.confirmed && tx.status.block_height
        ? currentHeight - tx.status.block_height + 1
        : 0;

    return {
      txid: tx.txid,
      confirmed: tx.status.confirmed,
      blockHeight: tx.status.block_height,
      blockHash: tx.status.block_hash,
      confirmations,
    };
  }

  async getCurrentBlockHeight(): Promise<number> {
    const response = await fetch(`${this.baseUrl}/blocks/tip/height`);
    const height = parseInt(await response.text());
    return height;
  }

  async getAddressUtxos(address: string): Promise<UTXO[]> {
    const response = await fetch(`${this.baseUrl}/address/${address}/utxo`);

    if (!response.ok) {
      throw new Error(
        `Failed to fetch UTXOs for ${address}: ${response.statusText}`
      );
    }

    return (await response.json()) as UTXO[];
  }

  async getTransactionHex(txid: string): Promise<string> {
    const response = await fetch(`${this.baseUrl}/tx/${txid}/hex`);

    if (!response.ok) {
      throw new Error(
        `Failed to fetch transaction hex: ${response.statusText}`
      );
    }

    return await response.text();
  }

  async broadcastTransaction(txHex: string): Promise<string> {
    const response = await fetch(`${this.baseUrl}/tx`, {
      method: 'POST',
      body: txHex,
    });

    if (!response.ok) {
      const error = await response.text();
      throw new Error(`Failed to broadcast transaction: ${error}`);
    }

    return await response.text();
  }

  /** Get API base URL for custom endpoints (/v1/fees/recommended, /mempool, etc) */
  getBaseUrl(): string {
    return this.baseUrl;
  }

  async isPrevoutSpent(txid: string, vout: number): Promise<boolean> {
    const response = await fetch(`${this.baseUrl}/tx/${txid}/outspends`);

    if (!response.ok) {
      if (response.status === 404) {
        // If the parent tx is unknown, treat as not spent yet
        return false;
      }
      throw new Error(
        `Failed to fetch outspends for ${txid}: ${response.statusText}`
      );
    }

    const outspends = (await response.json()) as Array<
      | {
          spent: boolean;
          txid: string;
          vin: number;
        }
      | null
    >;

    const info = outspends[vout];
    return info !== null && info !== undefined && info.spent === true;
  }
}
