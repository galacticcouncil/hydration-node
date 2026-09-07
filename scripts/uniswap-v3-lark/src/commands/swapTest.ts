import { config } from '../config'
import { connectApi, freeBalance, keyringFromSuri, signAndSend } from '../core/substrate'

interface Candidate {
  mid: number
  fee: number
  pool: string
}

function pick<T>(items: T[]): T {
  const chosen = items[Math.floor(Math.random() * items.length)]
  if (chosen === undefined) throw new Error('cannot pick from empty list')
  return chosen
}

export async function swapTest(): Promise<void> {
  const api = await connectApi(config.rpcWs)
  const call = api.call as any
  const events = api.events as any

  try {
    const signer = await keyringFromSuri(config.suri)
    console.log(`signer ${signer.address}`)

    const factory = (await api.query.parameters.uniswapV3Factory()) as any
    if (factory.isNone) throw new Error('uniswap v3 addresses not set — run `set-addresses`')
    console.log(`v3 factory ${factory.unwrap().toHex()}`)

    const keys = await api.query.omnipool.assets.keys()
    const omnipoolAssets = keys.map((k) => Number((k.args[0] as any).toString()))
    console.log(`omnipool assets: ${omnipoolAssets.join(', ')}`)
    if (!omnipoolAssets.includes(config.sellAssetIn)) {
      throw new Error(`SELL_ASSET_IN ${config.sellAssetIn} not in omnipool`)
    }

    const candidates: Candidate[] = []
    for (const mid of omnipoolAssets) {
      if (mid === config.quoteAsset || mid === config.sellAssetIn) continue
      for (const fee of config.fees) {
        const pool = (await call.uniswapV3Api.pool(mid, config.quoteAsset, fee)) as any
        if (pool.isSome) {
          const address = pool.unwrap().toHex()
          candidates.push({ mid, fee, pool: address })
          console.log(`  v3 pool: ${mid}/${config.quoteAsset} fee ${fee} -> ${address}`)
        }
      }
    }
    if (candidates.length === 0) throw new Error('no v3 pools discovered into the quote asset — run `seed`')

    const chosen = pick(candidates)
    console.log(
      `\nrandom path: ${config.sellAssetIn} --[Omnipool]--> ${chosen.mid} --[UniswapV3 ${chosen.fee}]--> ${config.quoteAsset}`,
    )

    const route = [
      { pool: 'Omnipool', assetIn: config.sellAssetIn, assetOut: chosen.mid },
      { pool: { UniswapV3: chosen.fee }, assetIn: chosen.mid, assetOut: config.quoteAsset },
    ]

    const before = await freeBalance(api, signer.address, config.quoteAsset)
    const result = await signAndSend(
      api,
      api.tx.router.sell(config.sellAssetIn, config.quoteAsset, config.sellAmount.toString(), '0', route as any),
      signer,
      `router.sell ${config.sellAmount} of ${config.sellAssetIn} -> ${config.quoteAsset}`,
    )
    const after = await freeBalance(api, signer.address, config.quoteAsset)

    let sawUniswap = false
    let sawOmnipool = false
    for (const { event } of result.events) {
      if (events.broadcast?.Swapped3?.is(event)) {
        const data = event.data as any
        const fillerType = data.fillerType ?? data[2]
        const filler = fillerType.type ?? fillerType.toString()
        if (filler === 'UniswapV3') sawUniswap = true
        if (filler === 'Omnipool') sawOmnipool = true
        console.log(`  Swapped3 filler ${filler}`)
      }
    }

    console.log(`\n${config.quoteAsset} balance: ${before} -> ${after} (delta ${after - before})`)
    console.log(`v3 leg emitted:       ${sawUniswap}`)
    console.log(`omnipool leg emitted: ${sawOmnipool}`)

    if (after > before && sawUniswap) {
      console.log('\nPASS: mixed swap routed through Uniswap v3')
      return
    }
    throw new Error('expected positive output and a UniswapV3 Swapped3 event')
  } finally {
    await api.disconnect()
  }
}
