import { ethers } from 'ethers'
import { CONFIG } from './config'
import { CryptoUtils } from './crypto-utils'
import { ProcessedTransaction } from './types'

export class TransactionProcessor {
  private static fundingProvider: ethers.JsonRpcProvider | null = null
  private static fundingProviderError: boolean = false
  static async processTransactionForSigning(
    rlpEncodedTx: Uint8Array,
    privateKey: string,
    slip44ChainId: number
  ): Promise<ProcessedTransaction> {
    console.log('\n🔐 Processing the Transaction for Signing')
    console.log('  📋 RLP-encoded transaction:', ethers.hexlify(rlpEncodedTx))
    console.log('  🔢 SLIP-44 Chain ID:', slip44ChainId)

    // Detect transaction type
    const isEIP1559 = rlpEncodedTx[0] === 0x02
    const txType = isEIP1559 ? 0x02 : 0x00
    const rlpData = isEIP1559 ? rlpEncodedTx.slice(1) : rlpEncodedTx

    console.log(`  📝 Transaction type: ${isEIP1559 ? 'EIP-1559' : 'Legacy'}`)

    // Create wallet and sign
    const wallet = new ethers.Wallet(privateKey)
    console.log('  👤 Signing address:', wallet.address)

    const unsignedTxHash = ethers.keccak256(rlpEncodedTx)
    const signature = wallet.signingKey.sign(unsignedTxHash)

    // Decode and prepare signed transaction
    const decoded = ethers.decodeRlp(rlpData) as string[]

    // Helper function to parse RLP integer fields
    const parseRlpInt = (value: string): number => {
      if (!value || value === '0x' || value === '0x80') {
        return 0
      }
      return parseInt(value, 16)
    }

    const nonce = isEIP1559 ? parseRlpInt(decoded[1]) : parseRlpInt(decoded[0])
    console.log('  📝 Transaction nonce:', nonce)
    const vValue = isEIP1559 ? signature.v - 27 : signature.v

    const signedFields = [
      ...decoded,
      ethers.toBeHex(vValue, 1),
      signature.r,
      signature.s,
    ]

    const signedRlp = ethers.encodeRlp(signedFields)
    const signedTransaction = isEIP1559
      ? ethers.concat([new Uint8Array([txType]), signedRlp])
      : signedRlp

    // Get correct transaction hash
    let signedTxHash: string
    try {
      const parsedTx = ethers.Transaction.from(signedTransaction)
      signedTxHash = parsedTx.hash!
    } catch {
      signedTxHash = ethers.keccak256(signedTransaction)
    }

    // Convert signature to Solana format
    const solanaSignature = await this.convertToSolanaSignature(signature)

    if (slip44ChainId === 60) {
      /// FUNDING DERIVED ADDRESS WITH ETH CODE
      const tx = ethers.Transaction.from(ethers.hexlify(rlpEncodedTx))
      const gasNeeded =
        tx.gasLimit * (tx.maxFeePerGas || tx.gasPrice!) + tx.value

      // Don't retry if we already know it's broken
      if (this.fundingProviderError) {
        console.error('Funding provider is unavailable, skipping balance check')
        return {
          unsignedTxHash,
          signedTxHash,
          signature: solanaSignature,
          signedTransaction: ethers.hexlify(signedTransaction),
          fromAddress: wallet.address,
          nonce,
        }
      }

      try {
        if (!this.fundingProvider) {
          const apiKey = process.env.SEPOLIA_RPC_URL
          if (!apiKey) {
            throw new Error('INFURA_API_KEY is not set')
          }

          const url = `${apiKey}`
          console.log(
            'Creating funding provider with URL:',
            url.replace(apiKey, 'xxx')
          )

          this.fundingProvider = new ethers.JsonRpcProvider(url)
          // Test the connection
          await this.fundingProvider.getNetwork()
        }

        const balance = await this.fundingProvider.getBalance(wallet.address)
        if (balance < gasNeeded) {
          const fundingWallet = new ethers.Wallet(
            process.env.PRIVATE_KEY_TESTNET!,
            this.fundingProvider
          )
          await fundingWallet
            .sendTransaction({
              to: wallet.address,
              value: gasNeeded - balance,
            })
            .then((tx) => tx.wait())
        }
      } catch (error) {
        console.error('Funding provider error:', error)
        this.fundingProviderError = true
        // Continue without funding - let the transaction fail naturally
      }
    }

    return {
      unsignedTxHash,
      signedTxHash,
      signature: solanaSignature,
      signedTransaction: ethers.hexlify(signedTransaction),
      fromAddress: wallet.address,
      nonce,
    }
  }

  private static async convertToSolanaSignature(
    signature: ethers.Signature
  ): Promise<any> {
    const rBigInt = BigInt(signature.r)
    const p = BigInt(CONFIG.SECP256K1_P)
    const ySquared = (rBigInt ** 3n + 7n) % p
    const y = CryptoUtils.modularSquareRoot(ySquared, p)
    const recoveryId = signature.v - 27
    const yParity = recoveryId
    const rY = y % 2n === BigInt(yParity) ? y : p - y

    return {
      bigR: {
        x: Array.from(Buffer.from(signature.r.slice(2), 'hex')),
        y: Array.from(Buffer.from(rY.toString(16).padStart(64, '0'), 'hex')),
      },
      s: Array.from(Buffer.from(signature.s.slice(2), 'hex')),
      recoveryId,
    }
  }
}
