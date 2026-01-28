import { ethers } from 'ethers';

export class RequestIdGenerator {
  /**
   * Generate a request ID for bidirectional sign operations
   *
   * Use this for sign-and-respond flows where:
   * - A transaction is signed on one chain (e.g., Ethereum, Bitcoin)
   * - The transaction is executed
   * - The result is monitored and returned to Solana
   *
   * @param sender - Solana public key of the requester
   * @param transactionData - Transaction identifier:
   *   - For Bitcoin: 32-byte txid (transaction hash without witness)
   *   - For EVM: Full RLP-encoded transaction bytes
   * @param caip2Id - CAIP-2 chain identifier (e.g., "eip155:1", "bip122:...")
   * @param keyVersion - MPC key version
   * @param path - Derivation path
   * @param algo - Signature algorithm
   * @param dest - Destination identifier
   * @param params - Additional parameters
   * @returns Deterministic request ID (keccak256 hash)
   */
  static generateSignBidirectionalRequestId(
    sender: string,
    transactionData: number[],
    caip2Id: string,
    keyVersion: number,
    path: string,
    algo: string,
    dest: string,
    params: string
  ): string {
    const txDataHex = '0x' + Buffer.from(transactionData).toString('hex');
    const encoded = ethers.solidityPacked(
      [
        'string',
        'bytes',
        'string',
        'uint32',
        'string',
        'string',
        'string',
        'string',
      ],
      [sender, txDataHex, caip2Id, keyVersion, path, algo, dest, params]
    );
    return ethers.keccak256(encoded);
  }

  /**
   * Generate a request ID for simple signature requests
   *
   * Use this for one-way signature operations where:
   * - A payload/message hash is signed
   * - Only the signature is returned (no transaction execution)
   * - No monitoring or bidirectional response needed
   *
   * @param addr - Solana public key of the requester
   * @param payload - Message hash or payload to sign
   * @param path - Derivation path
   * @param keyVersion - MPC key version
   * @param chainId - Chain identifier (number or string)
   * @param algo - Signature algorithm
   * @param dest - Destination identifier
   * @param params - Additional parameters
   * @returns Deterministic request ID (keccak256 hash)
   */
  static generateSignRequestId(
    addr: string,
    payload: number[],
    path: string,
    keyVersion: number,
    chainId: number | string,
    algo: string,
    dest: string,
    params: string
  ): string {
    const payloadHex = '0x' + Buffer.from(payload).toString('hex');
    const encoded = ethers.AbiCoder.defaultAbiCoder().encode(
      [
        'string',
        'bytes',
        'string',
        'uint32',
        'uint256',
        'string',
        'string',
        'string',
      ],
      [addr, payloadHex, path, keyVersion, chainId, algo, dest, params]
    );
    return ethers.keccak256(encoded);
  }

  /**
   * Generate a request ID for simple signature requests with string chain ID
   *
   * Use this for one-way signature operations where:
   * - A payload/message hash is signed
   * - Chain ID is provided as a string (e.g., "polkadot:2034")
   * - Used primarily for Substrate pallets
   *
   * @param addr - Account ID of the requester
   * @param payload - Message hash or payload to sign
   * @param path - Derivation path
   * @param keyVersion - MPC key version
   * @param chainId - Chain identifier as string (e.g., "polkadot:2034")
   * @param algo - Signature algorithm
   * @param dest - Destination identifier
   * @param params - Additional parameters
   * @returns Deterministic request ID (keccak256 hash)
   */
  static generateRequestIdStringChainId(
    addr: string,
    payload: number[],
    path: string,
    keyVersion: number,
    chainId: string,
    algo: string,
    dest: string,
    params: string
  ): string {
    const payloadHex = '0x' + Buffer.from(payload).toString('hex');
    const encoded = ethers.AbiCoder.defaultAbiCoder().encode(
      [
        'string',
        'bytes',
        'string',
        'uint32',
        'string', // ← Key difference: string instead of uint256
        'string',
        'string',
        'string',
      ],
      [addr, payloadHex, path, keyVersion, chainId, algo, dest, params]
    );
    return ethers.keccak256(encoded);
  }
}
