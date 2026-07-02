import { waitReady } from '@polkadot/wasm-crypto'
import {
  createApi,
  createKeyringAndAccounts,
  executeAsRoot,
  getTokenFree,
} from './utils'

async function main() {
  await waitReady()
  const api = await createApi()

  const faucetAsset = (api.consts.ethDispenser.faucetAsset as any).toNumber()
  const { alice } = createKeyringAndAccounts()

  const before = await getTokenFree(api, alice.address, faucetAsset)
  console.log(`\n[validate] Alice = ${alice.address}`)
  console.log(`[validate] asset ${faucetAsset} BEFORE = ${before.toString()}`)

  const mintAmount = 1_000_000_000_000_000n // 0.001 WETH (18 decimals)
  const mintCall = (api.tx as any).currencies.updateBalance(
    alice.address,
    faucetAsset,
    mintAmount.toString(),
  )

  console.log(`[validate] minting ${mintAmount} via executeAsRoot (TC → referendum fallback)...`)
  await executeAsRoot(api, alice, mintCall, 'VALIDATE mint faucet asset to Alice')

  const after = await getTokenFree(api, alice.address, faucetAsset)
  const delta = after - before
  console.log(`[validate] asset ${faucetAsset} AFTER  = ${after.toString()}`)
  console.log(`[validate] delta = ${delta.toString()}`)
  console.log(
    `[validate] RESULT = ${
      delta > 0n
        ? 'PASS ✅ balance increased — executeAsRoot applied the Root call'
        : 'NO-CHANGE ❌ Root call did not apply within the poll window (see log above)'
    }`,
  )

  await api.disconnect()
  process.exit(delta > 0n ? 0 : 2)
}

main().catch((e) => {
  console.error('[validate] FATAL', e)
  process.exit(1)
})
