import { Wallet } from 'ethers'
import { config } from '../config'
import { truncatedAccountId } from '../core/assets'
import { connectApi, freeBalance, keyringFromSuri, submitRootReferendum } from '../core/substrate'

export async function fund(): Promise<void> {
  const evmAddress = new Wallet(config.evmPrivateKey).address
  const dest = truncatedAccountId(evmAddress)
  console.log(`deployer EVM     ${evmAddress}`)
  console.log(`mapped substrate ${dest}`)

  const api = await connectApi(config.rpcWs)
  try {
    const signer = await keyringFromSuri(config.suri)
    console.log(`gov signer       ${signer.address}`)

    const verify = async (): Promise<boolean> => {
      const [gas, a, b] = await Promise.all([
        freeBalance(api, dest, config.gasAssetId),
        freeBalance(api, dest, config.tokenA),
        freeBalance(api, dest, config.tokenB),
      ])
      return gas >= config.fundGasAmount && a >= config.fundAmountA && b >= config.fundAmountB
    }

    if (await verify()) {
      console.log('deployer already funded')
      return
    }

    const calls = [
      api.tx.currencies.updateBalance(dest, config.gasAssetId, config.fundGasAmount.toString()),
      api.tx.currencies.updateBalance(dest, config.tokenA, config.fundAmountA.toString()),
      api.tx.currencies.updateBalance(dest, config.tokenB, config.fundAmountB.toString()),
    ]
    await submitRootReferendum(api, signer, api.tx.utility.batchAll(calls), {
      voteHdx: config.voteHdx,
      enactAfter: config.enactAfter,
      verify,
    })
    console.log('deployer funded')
  } finally {
    await api.disconnect()
  }
}
