import { config, requireDeployed } from '../config'
import { connectApi, keyringFromSuri, submitRootReferendum } from '../core/substrate'

export async function setAddresses(): Promise<void> {
  const { factory, swapRouter, quoter } = requireDeployed(config)

  const api = await connectApi(config.rpcWs)
  try {
    const signer = await keyringFromSuri(config.suri)
    console.log(`factory ${factory}`)
    console.log(`router  ${swapRouter}`)
    console.log(`quoter  ${quoter}`)

    const verify = async (): Promise<boolean> => {
      const stored = (await api.query.parameters.uniswapV3Factory()) as any
      return stored.isSome && stored.unwrap().toHex().toLowerCase() === factory.toLowerCase()
    }

    if (await verify()) {
      console.log('addresses already set')
      return
    }

    await submitRootReferendum(
      api,
      signer,
      api.tx.parameters.setUniswapV3Addresses(factory, swapRouter, quoter),
      { voteHdx: config.voteHdx, enactAfter: config.enactAfter, verify },
    )
    console.log('addresses set')
  } finally {
    await api.disconnect()
  }
}
