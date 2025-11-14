export interface PendingTransaction {
  txHash: string;
  requestId: string;
  chainId: number;
  explorerDeserializationFormat: number;
  explorerDeserializationSchema: any;
  callbackSerializationFormat: number;
  callbackSerializationSchema: any;
  sender: string;
  path: string;
  fromAddress: string;
  nonce: number;
  checkCount: number;
  source: string;
}

export interface ProcessedTransaction {
  unsignedTxHash: string;
  signedTxHash: string;
  signature: any;
  signedTransaction: string;
  fromAddress: string;
  nonce: number;
}

export interface TransactionOutput {
  success: boolean;
  output: any;
}

export interface TransactionStatus {
  status: "pending" | "success" | "error" | "fatal_error";
  success?: boolean;
  output?: any;
  reason?: string;
}
