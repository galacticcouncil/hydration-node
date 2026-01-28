import type * as anchor from '@coral-xyz/anchor';
import type { Program } from '@coral-xyz/anchor';
import type { PendingTransaction, ServerConfig, SignatureResponse } from '../../types';

/**
 * Context passed to bidirectional handlers for both Solana and Substrate.
 * Contains chain-agnostic dependencies and a signature sender abstraction.
 */
export interface BidirectionalHandlerContext {
  /** Chain-agnostic signature sender (Solana program or Substrate monitor) */
  sendSignatures: (requestIds: Uint8Array[], signatures: SignatureResponse[]) => Promise<void>;
  
  /** Server configuration (MPC keys, network settings, etc.) */
  config: ServerConfig;
  pendingTransactions: Map<string, PendingTransaction>;
  
  /** Source chain: 'solana' or 'polkadot' */
  source: 'solana' | 'polkadot';
}