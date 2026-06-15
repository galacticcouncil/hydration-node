import { JsonRpcProvider, Wallet, type ContractTransactionResponse } from 'ethers'

export function evmProvider(rpc: string): JsonRpcProvider {
  return new JsonRpcProvider(rpc, undefined, { batchMaxCount: 1 })
}

export function evmWallet(privateKey: string, provider: JsonRpcProvider): Wallet {
  return new Wallet(privateKey, provider)
}

export interface TxOverrides {
  gasLimit: bigint
  nonce: number
}

export class SequentialTxRunner {
  private nonce = -1

  constructor(
    private readonly wallet: Wallet,
    private readonly gasLimit: bigint,
  ) {}

  async init(): Promise<void> {
    const provider = this.wallet.provider
    if (!provider) throw new Error('wallet has no provider')
    this.nonce = await provider.getTransactionCount(this.wallet.address)
  }

  next(): TxOverrides {
    if (this.nonce < 0) throw new Error('SequentialTxRunner.init() not called')
    return { gasLimit: this.gasLimit, nonce: this.nonce++ }
  }

  async confirm(label: string, sent: Promise<ContractTransactionResponse>): Promise<void> {
    const tx = await sent
    const receipt = await tx.wait()
    if (!receipt || receipt.status !== 1) {
      throw new Error(`${label} reverted (tx ${tx.hash})`)
    }
    console.log(`   ${label} tx ${receipt.hash} status ${receipt.status}`)
  }
}
