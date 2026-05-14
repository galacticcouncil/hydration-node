import { ethers } from "ethers";

export class TransactionBuilder {
  static buildEIP1559(params: {
    chainId: number;
    nonce: number;
    maxPriorityFeePerGas: bigint;
    maxFeePerGas: bigint;
    gasLimit: number;
    to: string;
    value: bigint;
    data: string;
    accessList: any[];
  }): { transaction: ethers.Transaction; serialized: number[]; unwrapped: number[] } {
    const transaction = ethers.Transaction.from({
      type: 2,
      ...params
    });
    
    const fullSerialized = Array.from(ethers.getBytes(transaction.unsignedSerialized));
    
    return {
      transaction,
      serialized: fullSerialized,
      unwrapped: fullSerialized.slice(1)
    };
  }
}