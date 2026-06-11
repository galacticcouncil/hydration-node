import { ApiPromise, WsProvider } from '@polkadot/api'
import { Keyring } from '@polkadot/keyring'
import type { KeyringPair } from '@polkadot/keyring/types'
import type { SubmittableExtrinsic } from '@polkadot/api/types'
import type { ISubmittableResult } from '@polkadot/types/types'
import { cryptoWaitReady } from '@polkadot/util-crypto'

const HDX_DECIMALS = 12n

export async function connectApi(ws: string): Promise<ApiPromise> {
  if (!ws) throw new Error('empty ws endpoint')
  const api = await ApiPromise.create({ provider: new WsProvider(ws) })
  await api.isReady
  return api
}

export async function keyringFromSuri(suri: string): Promise<KeyringPair> {
  await cryptoWaitReady()
  return new Keyring({ type: 'sr25519', ss58Format: 63 }).addFromUri(suri)
}

export async function freeBalance(api: ApiPromise, address: string, assetId: number): Promise<bigint> {
  if (assetId === 0) {
    const account = (await api.query.system.account(address)) as any
    return BigInt(account.data.free.toString())
  }
  const account = (await api.query.tokens.accounts(address, assetId)) as any
  return BigInt(account.free.toString())
}

export function decodeError(api: ApiPromise, error: any): string {
  if (error.isModule) {
    const meta = api.registry.findMetaError(error.asModule)
    return `${meta.section}.${meta.name}: ${meta.docs.join(' ')}`
  }
  return error.toString()
}

export function signAndSend(
  api: ApiPromise,
  tx: SubmittableExtrinsic<'promise'>,
  signer: KeyringPair,
  label: string,
): Promise<ISubmittableResult> {
  console.log(`-> ${label}`)
  return new Promise((resolve, reject) => {
    tx.signAndSend(signer, (result) => {
      const { status, dispatchError } = result
      if (dispatchError) {
        reject(new Error(decodeError(api, dispatchError)))
        return
      }
      if (status.isInBlock) {
        console.log(`   in block ${status.asInBlock.toHex()}`)
        resolve(result)
      }
    }).catch(reject)
  })
}

export const sleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms))

export interface ReferendumOptions {
  voteHdx: bigint
  enactAfter: number
  verify?: () => Promise<boolean>
}

export async function submitRootReferendum(
  api: ApiPromise,
  signer: KeyringPair,
  inner: SubmittableExtrinsic<'promise'>,
  opts: ReferendumOptions,
): Promise<number> {
  const callHex = inner.method.toHex()
  const lenBytes = (callHex.length - 2) / 2

  let proposal: Record<string, unknown>
  if (lenBytes <= 100) {
    proposal = { Inline: callHex }
  } else {
    try {
      await signAndSend(api, api.tx.preimage.notePreimage(callHex), signer, `Note preimage (${lenBytes} bytes)`)
    } catch (e) {
      if (!String(e).includes('AlreadyNoted')) throw e
      console.log('   preimage already noted')
    }
    proposal = { Lookup: { hash: inner.method.hash.toHex(), len: lenBytes } }
  }

  const submitted = await signAndSend(
    api,
    api.tx.referenda.submit({ system: 'Root' }, proposal as any, { After: opts.enactAfter }),
    signer,
    'Submit Root referendum',
  )

  let refIndex: number | null = null
  for (const { event } of submitted.events) {
    if (event.section === 'referenda' && event.method === 'Submitted') {
      const raw = event.data[0]
      if (raw) refIndex = Number(raw.toString())
      break
    }
  }
  if (refIndex === null) throw new Error('no referenda.Submitted event')
  console.log(`   referendum index ${refIndex}`)

  await signAndSend(api, api.tx.referenda.placeDecisionDeposit(refIndex), signer, 'Place decision deposit')
  await signAndSend(
    api,
    api.tx.convictionVoting.vote(refIndex, {
      Standard: { vote: { aye: true, conviction: 'None' }, balance: opts.voteHdx * 10n ** HDX_DECIMALS },
    }),
    signer,
    `Vote aye with ${opts.voteHdx} HDX`,
  )

  console.log('   waiting for approval...')
  let approved = false
  for (let i = 0; i < 80; i++) {
    const info = (await api.query.referenda.referendumInfoFor(refIndex)).toHuman() as Record<string, unknown> | null
    if (info?.['Approved']) {
      approved = true
      break
    }
    if (info?.['Rejected'] || info?.['Cancelled'] || info?.['TimedOut'] || info?.['Killed']) {
      throw new Error(`referendum ended without approval: ${JSON.stringify(info)}`)
    }
    await sleep(6000)
  }
  if (!approved) throw new Error('referendum not approved within timeout')

  if (opts.verify) {
    console.log('   waiting for enactment...')
    for (let i = 0; i < 40; i++) {
      if (await opts.verify()) {
        console.log('   effect confirmed')
        return refIndex
      }
      await sleep(6000)
    }
    throw new Error('referendum approved but effect not observed within timeout')
  }
  return refIndex
}
