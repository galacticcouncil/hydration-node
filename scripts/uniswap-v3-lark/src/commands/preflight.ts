import { Contract } from 'ethers'
import { config } from '../config'
import { FACTORY_ABI } from '../core/abis'
import { assetToEvmAddress, sortTokens } from '../core/assets'
import { evmProvider } from '../core/evm'
import { connectApi, freeBalance, keyringFromSuri } from '../core/substrate'

const mark = (ready: boolean): string => (ready ? 'OK ' : 'XX ')

export async function preflight(): Promise<void> {
  const api = await connectApi(config.rpcWs)
  const call = api.call as any
  const tx = api.tx as any
  const events = api.events as any

  try {
    const signer = await keyringFromSuri(config.suri)

    console.log('=== runtime metadata ===')
    console.log(`${mark(!!call.uniswapV3Api?.pool)} api.call.uniswapV3Api.pool`)
    console.log(`${mark(!!tx.parameters?.setUniswapV3Addresses)} tx.parameters.setUniswapV3Addresses`)
    console.log(`${mark(!!tx.router?.sell)} tx.router.sell`)
    console.log(`${mark(!!events.broadcast?.Swapped3)} events.broadcast.Swapped3`)

    console.log('=== parameters ===')
    const factory = (await api.query.parameters.uniswapV3Factory()) as any
    const router = (await api.query.parameters.uniswapV3SwapRouter()) as any
    const quoter = (await api.query.parameters.uniswapV3Quoter()) as any
    console.log(`factory: ${factory.isSome ? factory.unwrap().toHex() : 'NOT SET'}`)
    console.log(`router : ${router.isSome ? router.unwrap().toHex() : 'NOT SET'}`)
    console.log(`quoter : ${quoter.isSome ? quoter.unwrap().toHex() : 'NOT SET'}`)

    console.log('=== omnipool leg ===')
    for (const id of [config.sellAssetIn, config.tokenA]) {
      const state = (await api.query.omnipool.assets(id)) as any
      console.log(`asset ${id}: ${state.isSome ? `tradable ${state.unwrap().tradable.toString()}` : 'NOT IN OMNIPOOL'}`)
    }

    console.log('=== signer ===')
    console.log(`${signer.address} HDX ${await freeBalance(api, signer.address, 0)}`)

    if (factory.isSome) {
      console.log('=== runtime quote (QuoterV2 via staticcall) ===')
      const pool = (await call.uniswapV3Api.pool(config.tokenA, config.quoteAsset, config.fee)) as any
      console.log(`pool(${config.tokenA},${config.quoteAsset},${config.fee}): ${pool.isSome ? pool.unwrap().toHex() : 'NONE'}`)
      const probe = config.amountA / 1000n
      const out = (await call.uniswapV3Api.quoteSell(
        config.tokenA,
        config.quoteAsset,
        config.fee,
        probe.toString(),
      )) as any
      console.log(`quoteSell ${probe} ${config.tokenA}->${config.quoteAsset}: ${out.isSome ? out.unwrap().toString() : 'NONE'}`)
    }

    if (config.factory) {
      console.log('=== v3 pool (EVM) ===')
      const provider = evmProvider(config.rpcEvm)
      const factoryContract = new Contract(config.factory, FACTORY_ABI, provider)
      const [t0, t1] = sortTokens(assetToEvmAddress(config.tokenA), assetToEvmAddress(config.tokenB))
      console.log(`getPool: ${await factoryContract.getPool(t0, t1, config.fee)}`)
    }
  } finally {
    await api.disconnect()
  }
}
