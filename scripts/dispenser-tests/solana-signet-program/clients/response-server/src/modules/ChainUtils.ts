/**
 * Utilities for chain-specific operations
 */

export enum SerializationFormat {
  Borsh = 0,
  ABI = 1,
  BitcoinSimple = 2,
}

/**
 * Extract namespace from CAIP-2 chain ID
 *
 * @param caip2Id - CAIP-2 chain identifier
 * @returns namespace - The chain namespace
 *
 * @example
 * getNamespaceFromCaip2("eip155:1") // "eip155" (Ethereum)
 * getNamespaceFromCaip2("solana:mainnet") // "solana"
 * getNamespaceFromCaip2("bip122:000000000000000000064...") // "bip122" (Bitcoin)
 */
export function getNamespaceFromCaip2(caip2Id: string): string {
  const [namespace] = caip2Id.split(':');
  if (!namespace) {
    throw new Error(`Invalid CAIP-2 ID: ${caip2Id}`);
  }
  return namespace.toLowerCase();
}

/**
 * Get serialization format from CAIP-2 chain ID
 *
 * @param caip2Id - CAIP-2 chain identifier
 * @returns SerializationFormat enum value
 *
 * Formats:
 * - Borsh (0): Solana chains
 * - ABI (1): EVM chains (Ethereum, Polygon, BSC, etc.)
 * - BitcoinSimple (2): Bitcoin (testnet4/regtest) - returns { success: bool } only
 *
 * @example
 * getSerializationFormat("eip155:1") // SerializationFormat.ABI
 * getSerializationFormat("solana:mainnet") // SerializationFormat.Borsh
 * getSerializationFormat("bip122:000000000000000000064...") // SerializationFormat.BitcoinSimple
 */
export function getSerializationFormat(caip2Id: string): SerializationFormat {
  const namespace = getNamespaceFromCaip2(caip2Id);

  switch (namespace) {
    case 'eip155':
      return SerializationFormat.ABI;
    case 'solana':
      return SerializationFormat.Borsh;
    case 'bip122':
      return SerializationFormat.BitcoinSimple;
    default:
      throw new Error(`Unsupported chain namespace: ${namespace}`);
  }
}
