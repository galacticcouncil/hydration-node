import { Wallet } from 'ethers'
import { config } from '../config'
import { connectApi, keyringFromSuri, submitRootReferendum } from '../core/substrate'

export async function whitelist(): Promise<void> {
  const evmAddress = new Wallet(config.evmPrivateKey).address
  console.log(`deployer ${evmAddress}`)

  const api = await connectApi(config.rpcWs)
  try {
    const signer = await keyringFromSuri(config.suri)
    const verify = async (): Promise<boolean> =>
      ((await api.query.evmAccounts.contractDeployer(evmAddress)) as any).isSome

    if (await verify()) {
      console.log('already whitelisted')
      return
    }

    await submitRootReferendum(api, signer, api.tx.evmAccounts.addContractDeployer(evmAddress), {
      voteHdx: config.voteHdx,
      enactAfter: config.enactAfter,
      verify,
    })
    console.log('deployer whitelisted')
  } finally {
    await api.disconnect()
  }
}
