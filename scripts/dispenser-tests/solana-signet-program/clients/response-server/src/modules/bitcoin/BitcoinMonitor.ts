import type {
  TransactionStatus,
  TransactionOutputData,
  ServerConfig,
  PrevoutRef,
} from '../../types';
import { IBitcoinAdapter } from '../../adapters/IBitcoinAdapter';
import { BitcoinAdapterFactory } from '../../adapters/BitcoinAdapterFactory';

/**
 * Bitcoin transaction monitor using adapter pattern
 *
 * Supports:
 * - mempool.space API (testnet)
 * - Bitcoin Core RPC (regtest)
 *
 * Automatically selects adapter based on network:
 * - regtest → Bitcoin Core RPC adapter (localhost:18443)
 * - testnet → mempool.space API (testnet4)
 *
 * Features:
 * - Structured output object `{ success: true, isFunctionCall: false }`
 *
 * Note: Transactions are broadcast by the client, not the server.
 */
export class BitcoinMonitor {
  private static adapterCache = new Map<string, IBitcoinAdapter>();

  /**
   * Poll a Bitcoin transaction until it confirms or a conflict is detected.
   *
   * - Selects the correct adapter (regtest via RPC, testnet via mempool.space).
   * - Returns `pending` while waiting for confirmations.
   * - Returns `success` once the minimum confirmation threshold is met.
   * - Returns `error` if any prevout is spent elsewhere (double-spend).
   *
   * @param txid Explorer-facing txid (big-endian).
   * @param prevouts Optional prevouts consumed by this tx, for conflict checks.
   * @param config Server config (chooses network and adapter).
   */
  static async waitForTransactionAndGetOutput(
    txid: string,
    prevouts: PrevoutRef[] | undefined,
    config: ServerConfig
  ): Promise<TransactionStatus> {
    const adapter = await this.getAdapter(config);
    const requiredConfs = 1;

    try {
      const tx = await adapter.getTransaction(txid);

      if (tx.confirmations < requiredConfs) {
        const conflicted = await this.getConflictedPrevout(
          prevouts,
          adapter
        );
        if (conflicted) {
          console.error(
            `❌ ${config.bitcoinNetwork} tx ${txid}: input ${conflicted.txid}:${conflicted.vout} was spent elsewhere`
          );
          return { status: 'error', reason: 'inputs_spent' };
        }

        const hint = `${tx.confirmations}/${requiredConfs} confirmations`;

        console.log(
          `⏳ ${config.bitcoinNetwork} tx ${txid}: ${hint}`
        );
        return { status: 'pending' };
      }

      console.log(
        `✅ ${config.bitcoinNetwork} tx ${txid}: ${tx.confirmations} confirmation(s)`
      );

      const output: TransactionOutputData = {
        success: true,
        isFunctionCall: false,
      };

      return {
        status: 'success',
        success: true,
        output,
      };
    } catch (error) {
      if (error instanceof Error && error.message.includes('not found')) {
        const conflicted = await this.getConflictedPrevout(
          prevouts,
          adapter
        );
        if (conflicted) {
          console.error(
            `❌ ${config.bitcoinNetwork} tx ${txid}: input ${conflicted.txid}:${conflicted.vout} was spent elsewhere`
          );
          return { status: 'error', reason: 'inputs_spent' };
        }

        console.log(`⏳ ${config.bitcoinNetwork} tx ${txid}: not found`);
        return { status: 'pending' };
      }

      console.error(
        `❌ Error while monitoring ${txid}: ${
          error instanceof Error ? error.message : String(error)
        }`
      );
      return { status: 'pending' };
    }
  }

  private static async getAdapter(
    config: ServerConfig
  ): Promise<IBitcoinAdapter> {
    const network = config.bitcoinNetwork;

    if (this.adapterCache.has(network)) {
      return this.adapterCache.get(network)!;
    }

    const adapter = await BitcoinAdapterFactory.create(network);

    this.adapterCache.set(network, adapter);
    return adapter;
  }

  /**
   * Check if any prevout has been spent in another transaction.
   *
   * @param prevouts List of txid/vout pairs from the original PSBT inputs.
   * @param adapter Active Bitcoin adapter (RPC or mempool.space).
   * @returns First conflicting prevout, or null if none are spent.
   */
  private static async getConflictedPrevout(
    prevouts: PrevoutRef[] | undefined,
    adapter: IBitcoinAdapter
  ): Promise<PrevoutRef | null> {
    if (!prevouts || prevouts.length === 0) {
      return null;
    }

    for (const prev of prevouts) {
      try {
        const spent = await adapter.isPrevoutSpent(prev.txid, prev.vout);
        if (spent) {
          return prev;
        }
      } catch (error) {
        console.error(
          `❌ Error checking prevout ${prev.txid}:${prev.vout}: ${
            error instanceof Error ? error.message : String(error)
          }`
        );
      }
    }

    return null;
  }
}
