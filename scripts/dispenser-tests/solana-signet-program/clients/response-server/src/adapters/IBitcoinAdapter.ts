/**
 * Bitcoin transaction information with confirmation status
 */
export interface BitcoinTransactionInfo {
  txid: string;
  confirmed: boolean;
  blockHeight?: number;
  blockHash?: string;
  confirmations: number;
}

/**
 * Unspent Transaction Output
 *
 * Value is always in satoshis (1 BTC = 100,000,000 sats)
 */
export interface UTXO {
  txid: string;
  vout: number;
  value: number;
  status?: {
    confirmed: boolean;
    block_height?: number;
  };
}

/**
 * Bitcoin adapter - unified interface for regtest and testnet
 *
 * Address formats: tb1q... (testnet), bcrt1q... (regtest)
 * All amounts are in satoshis (1 BTC = 100,000,000 sats)
 */
export interface IBitcoinAdapter {
  /**
   * Get transaction information including confirmation status
   * @param txid Transaction ID (hex string)
   * @returns Transaction info with confirmations
   */
  getTransaction(txid: string): Promise<BitcoinTransactionInfo>;

  /**
   * Get current blockchain height
   * @returns Current block height
   */
  getCurrentBlockHeight(): Promise<number>;

  /**
   * Check if Bitcoin backend is available
   * @returns True if backend is reachable
   */
  isAvailable(): Promise<boolean>;

  /**
   * Get unspent transaction outputs for an address
   * @param address Bitcoin address (tb1q... or bcrt1q...)
   * @returns Array of UTXOs with values in satoshis
   */
  getAddressUtxos(address: string): Promise<UTXO[]>;

  /**
   * Get raw transaction as hex string
   * @param txid Transaction ID
   * @returns Raw transaction hex
   */
  getTransactionHex(txid: string): Promise<string>;

  /**
   * Broadcast signed transaction to network
   * @param txHex Raw transaction hex string
   * @returns Transaction ID of broadcast transaction
   */
  broadcastTransaction(txHex: string): Promise<string>;

  /**
   * Mine blocks (regtest only)
   * @param count Number of blocks to mine
   * @param address Optional mining reward address (generates new address if omitted)
   * @returns Array of block hashes
   */
  mineBlocks?(count: number, address?: string): Promise<string[]>;

  /**
   * Fund address and confirm transaction (regtest only)
   * @param address Recipient address
   * @param amount Amount in BTC (not satoshis)
   * @returns Transaction ID
   */
  fundAddress?(address: string, amount: number): Promise<string>;

  /**
   * Check if a specific prevout has already been spent
   * @param txid Source transaction ID of the UTXO
   * @param vout Output index within the transaction
   * @returns True if the UTXO is spent
   */
  isPrevoutSpent(txid: string, vout: number): Promise<boolean>;
}
