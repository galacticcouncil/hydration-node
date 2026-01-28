import { ethers } from 'ethers';
import {
  ProcessedTransaction,
  ServerConfig,
  SignatureResponse,
} from '../../types';
import { getNamespaceFromCaip2 } from '../ChainUtils';

export class EthereumTransactionProcessor {
  private static fundingProvider: ethers.JsonRpcProvider | null = null;
  private static fundingProviderError: boolean = false;

  static async processTransactionForSigning(
    rlpEncodedTx: Uint8Array,
    privateKey: string,
    caip2Id: string,
    config: ServerConfig
  ): Promise<ProcessedTransaction> {
    console.log('\n🔐 Processing the Transaction for Signing');
    console.log('  📋 RLP-encoded transaction:', ethers.hexlify(rlpEncodedTx));
    console.log('  🔗 Chain ID:', caip2Id);

    // Detect transaction type
    const isEIP1559 = rlpEncodedTx[0] === 0x02;
    const txType = isEIP1559 ? 0x02 : 0x00;
    const rlpData = isEIP1559 ? rlpEncodedTx.slice(1) : rlpEncodedTx;

    console.log(`  📝 Transaction type: ${isEIP1559 ? 'EIP-1559' : 'Legacy'}`);

    // Create wallet and sign
    const wallet = new ethers.Wallet(privateKey);
    console.log('  👤 Signing address:', wallet.address);

    const unsignedTxHash = ethers.keccak256(rlpEncodedTx);
    const signature = wallet.signingKey.sign(unsignedTxHash);

    // Decode and prepare signed transaction
    const decoded = ethers.decodeRlp(rlpData) as string[];
    const nonce = isEIP1559
      ? parseInt(decoded[1], 16) // Second field for EIP-1559
      : parseInt(decoded[0], 16); // First field for legacy
    console.log(' 📝 Transaction nonce:', nonce);
    const vValue = isEIP1559 ? signature.v - 27 : signature.v;

    const signedFields = [
      ...decoded,
      ethers.toBeHex(vValue, 1),
      signature.r,
      signature.s,
    ];

    const signedRlp = ethers.encodeRlp(signedFields);
    const signedTransaction = isEIP1559
      ? ethers.concat([new Uint8Array([txType]), signedRlp])
      : signedRlp;

    // Get correct transaction hash
    let signedTxHash: string;
    try {
      const parsedTx = ethers.Transaction.from(signedTransaction);
      signedTxHash = parsedTx.hash!;
    } catch {
      signedTxHash = ethers.keccak256(signedTransaction);
    }

    // Convert signature to Solana format (single signature for EVM transactions)
    const solanaSignature = this.toSolanaSignature(signature);

    const namespace = getNamespaceFromCaip2(caip2Id);
    if (namespace === 'eip155') {
      /// FUNDING DERIVED ADDRESS WITH ETH CODE
      const tx = ethers.Transaction.from(ethers.hexlify(rlpEncodedTx));
      const gasNeeded =
        tx.gasLimit * (tx.maxFeePerGas || tx.gasPrice!) + tx.value;

      // Don't retry if we already know it's broken
      if (this.fundingProviderError) {
        console.error(
          'Funding provider is unavailable, skipping balance check'
        );
        return {
          signedTxHash,
          signature: [solanaSignature], // EVM has single signature
          signedTransaction: ethers.hexlify(signedTransaction),
          fromAddress: wallet.address,
          nonce,
        };
      }

      try {
        if (!this.fundingProvider) {
          const url = config.rpcUrl;
          this.fundingProvider = new ethers.JsonRpcProvider(url);
          await this.fundingProvider.getNetwork();
        }

        const balance = await this.fundingProvider.getBalance(wallet.address);
        if (balance < gasNeeded) {
          const fundingWallet = new ethers.Wallet(
            config.mpcRootKey,
            this.fundingProvider
          );
          await fundingWallet
            .sendTransaction({
              to: wallet.address,
              value: gasNeeded - balance,
            })
            .then((tx) => tx.wait());
        }
      } catch (error) {
        console.error('Funding provider error:', error);
        this.fundingProviderError = true;
      }
    }

    return {
      signedTxHash,
      signature: [solanaSignature], // EVM has single signature
      signedTransaction: ethers.hexlify(signedTransaction),
      fromAddress: wallet.address,
      nonce,
    };
  }

  private static toSolanaSignature(
    signature: ethers.Signature
  ): SignatureResponse {
    const prefix = signature.yParity === 0 ? '02' : '03';
    const compressed = prefix + signature.r.slice(2);
    const point = ethers.SigningKey.computePublicKey('0x' + compressed, false);
    const pointBytes = ethers.getBytes(point);

    return {
      bigR: {
        x: Array.from(Buffer.from(signature.r.slice(2), 'hex')),
        y: Array.from(pointBytes.slice(33, 65)),
      },
      s: Array.from(Buffer.from(signature.s.slice(2), 'hex')),
      recoveryId: signature.yParity || 0,
    };
  }
}
