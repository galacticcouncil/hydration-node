import { ethers } from 'ethers'
import { CONFIG } from './config'
import { TransactionOutput, TransactionStatus } from './types'

export class EthereumMonitor {
  private static providerCache = new Map<string, any>()
  static async waitForTransactionAndGetOutput(
    txHash: string,
    slip44ChainId: number,
    explorerDeserializationFormat: number,
    explorerDeserializationSchema: any,
    fromAddress: string,
    nonce: number
  ): Promise<TransactionStatus> {
    let provider: ethers.JsonRpcProvider

    try {
      console.log(slip44ChainId)
      provider = this.getProvider(slip44ChainId)
    } catch (e) {
      return { status: 'fatal_error', reason: 'unsupported_chain' }
    }

    console.log(`⏳ Checking transaction ${txHash}...`)

    try {
      const receipt = await provider.getTransactionReceipt(txHash)

      if (receipt) {
        // Transaction is mined!
        console.log(`✅ Transaction found! Confirmation complete.`)
        console.log(`  📦 Block number: ${receipt.blockNumber}`)
        console.log(
          `  ${receipt.status === 1 ? '✅' : '❌'} Status: ${
            receipt.status === 1 ? 'Success' : 'Failed'
          }`
        )

        if (receipt.status === 0) {
          return { status: 'error', reason: 'reverted' }
        }

        // Get transaction for output extraction
        const tx = await provider.getTransaction(txHash)
        if (!tx) {
          return { status: 'pending' }
        }

        try {
          const output = await this.extractTransactionOutput(
            tx,
            receipt,
            provider,
            explorerDeserializationFormat,
            explorerDeserializationSchema,
            fromAddress
          )
          return {
            status: 'success',
            success: output.success,
            output: output.output,
          }
        } catch (e) {
          return { status: 'fatal_error', reason: 'extraction_failed' }
        }
      } else {
        // No receipt - check if replaced
        const currentNonce = await provider.getTransactionCount(fromAddress)
        if (currentNonce > nonce) {
          // Check if it was our transaction
          const receiptCheck = await provider.getTransactionReceipt(txHash)
          if (!receiptCheck) {
            return { status: 'error', reason: 'replaced' }
          }
        }

        // Check if transaction exists
        const tx = await provider.getTransaction(txHash)
        if (!tx) {
          return { status: 'pending' }
        }

        console.log(`✅ Transaction found! Waiting for confirmation...`)

        // Already checked receipt above and it was null, so return pending
        return { status: 'pending' }
      }
    } catch (e) {
      return { status: 'pending' }
    }
  }

  private static getProvider(slip44ChainId: number): ethers.JsonRpcProvider {
    const rpcUrl = process.env.RPC_URL || 'https://api.devnet.solana.com'
    const isDevnet = rpcUrl.includes('devnet')
    const cacheKey = `${slip44ChainId}-${isDevnet}`

    if (this.providerCache.has(cacheKey)) {
      return this.providerCache.get(cacheKey)!
    }

    let url: string
    switch (slip44ChainId) {
      case 60: // Ethereum
        if (isDevnet) {
          url = process.env.SEPOLIA_RPC_URL || `${process.env.INFURA_API_KEY}`
          console.log('  🌐 Using Ethereum Sepolia')
        } else {
          url = process.env.ETHEREUM_RPC_URL || 'https://eth.llamarpc.com'
          console.log('  🌐 Using Ethereum Mainnet')
        }
        break
      default:
        throw new Error(`Unsupported SLIP-44 chain ID: ${slip44ChainId}`)
    }

    const provider = new ethers.JsonRpcProvider(url)
    this.providerCache.set(cacheKey, provider)
    return provider
  }

  private static async extractTransactionOutput(
    tx: ethers.TransactionResponse,
    receipt: ethers.TransactionReceipt,
    provider: ethers.JsonRpcProvider,
    explorerDeserializationFormat: number,
    explorerDeserializationSchema: any,
    fromAddress: string
  ): Promise<TransactionOutput> {
    const isContractCall = tx.data && tx.data !== '0x' && tx.data.length > 2

    if (isContractCall && explorerDeserializationFormat === 1) {
      try {
        console.log('  📞 Getting function return value...')

        const callResult = await provider.call({
          to: tx.to,
          data: tx.data,
          from: fromAddress,
          blockTag: receipt.blockNumber - 1,
        })

        const schemaStr =
          typeof explorerDeserializationSchema === 'string'
            ? explorerDeserializationSchema
            : new TextDecoder().decode(
                new Uint8Array(explorerDeserializationSchema)
              )

        const schema = JSON.parse(schemaStr)
        const decoded = ethers.AbiCoder.defaultAbiCoder().decode(
          schema.map((s: any) => s.type),
          callResult
        )

        const decodedOutput: any = {}
        schema.forEach((field: any, index: number) => {
          decodedOutput[field.name] = decoded[index]
        })

        console.log('  📊 Decoded output:', decodedOutput)
        return { success: true, output: decodedOutput }
      } catch (e) {
        console.error('  ⚠️ Error getting function return value:', e)
        return { success: true, output: { success: true } }
      }
    } else {
      return {
        success: true,
        output: {
          success: true,
          isFunctionCall: false,
        },
      }
    }
  }
}
