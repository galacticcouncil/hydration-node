import {
  IBitcoinAdapter,
  BitcoinTransactionInfo,
  UTXO,
} from './IBitcoinAdapter';
import Client from 'bitcoin-core';

/**
 * Bitcoin Core RPC adapter for regtest (localhost:18443)
 *
 * Uses getrawtransaction (not gettransaction) to query any tx, not just wallet txs.
 * Converts BTC amounts to/from satoshis automatically.
 *
 * Setup: https://github.com/Pessina/bitcoin-regtest
 */
export class BitcoinCoreRpcAdapter implements IBitcoinAdapter {
  private client: Client;

  constructor(client: Client) {
    this.client = client;
  }

  async isAvailable(): Promise<boolean> {
    try {
      await this.client.command('getblockchaininfo');
      return true;
    } catch (error) {
      return false;
    }
  }

  async getTransaction(txid: string): Promise<BitcoinTransactionInfo> {
    try {
      // Use getrawtransaction with verbose=true to get ANY transaction
      // (not just wallet transactions like gettransaction does)
      const tx = await this.client.command('getrawtransaction', txid, true);

      return {
        txid: tx.txid,
        confirmed: tx.confirmations > 0,
        blockHeight: tx.blockheight,
        blockHash: tx.blockhash,
        confirmations: tx.confirmations || 0,
      };
    } catch (error) {
      if (
        error instanceof Error &&
        (error.message.includes('No such mempool or blockchain transaction') ||
          error.message.includes('Invalid or non-wallet transaction'))
      ) {
        throw new Error(`Transaction ${txid} not found`);
      }
      throw error;
    }
  }

  async getCurrentBlockHeight(): Promise<number> {
    return await this.client.command('getblockcount');
  }

  async getAddressUtxos(address: string): Promise<UTXO[]> {
    const result = await this.client.command('scantxoutset', 'start', [
      `addr(${address})`,
    ]);

    return result.unspents.map((utxo: any) => ({
      txid: utxo.txid,
      vout: utxo.vout,
      value: Math.round(utxo.amount * 100000000),
      status: {
        confirmed: true,
      },
    }));
  }

  async getTransactionHex(txid: string): Promise<string> {
    return await this.client.command('getrawtransaction', txid, false);
  }

  async broadcastTransaction(txHex: string): Promise<string> {
    const hash = await this.client.command('sendrawtransaction', txHex);
    await this.mineBlocks(1);
    return hash;
  }

  async mineBlocks(count: number, address?: string): Promise<string[]> {
    const minerAddress =
      address || (await this.client.command('getnewaddress'));

    return await this.client.command('generatetoaddress', count, minerAddress);
  }

  async fundAddress(address: string, amount: number): Promise<string> {
    const txid = await this.client.command('sendtoaddress', address, amount);
    await this.mineBlocks(1);
    return txid;
  }

  /** Get underlying RPC client for advanced operations (getblockchaininfo, etc) */
  getClient(): Client {
    return this.client;
  }

  static createRegtestAdapter(): BitcoinCoreRpcAdapter {
    const client = new Client({
      host: 'http://localhost:18443',
      username: 'test',
      password: 'test123',
    });

    return new BitcoinCoreRpcAdapter(client);
  }

  async isPrevoutSpent(txid: string, vout: number): Promise<boolean> {
    const utxo = await this.client.command('gettxout', txid, vout, true);
    return utxo === null;
  }
}
