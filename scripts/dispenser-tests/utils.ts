import { ApiPromise, WsProvider, Keyring } from '@polkadot/api'
import { ISubmittableResult } from '@polkadot/types/types'
import { encodeAddress } from '@polkadot/keyring'
import { ethers } from 'ethers'
import { KeyDerivation } from './key-derivation'
import { blake2AsHex } from '@polkadot/util-crypto'
import { SubmittableExtrinsic } from '@polkadot/api/types'
import { ENV } from './env'

// --- Substrate funding thresholds (not network-specific) ---
export const MIN_BOB_NATIVE_BALANCE = 1
export const PALLET_MIN_NATIVE_BALANCE = 10_000_000_000_000n
export const BOB_NATIVE_TOPUP = 100_000_000_000_000n
export const PALLET_FAUCET_FUND = ethers.parseEther('100')

export const PALLET_ID_STR = 'py/fucet'
export const MODL_PREFIX = 'modl'

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

export function getPalletAccountId(): Uint8Array {
  const palletId = new TextEncoder().encode(PALLET_ID_STR)
  const modl = new TextEncoder().encode(MODL_PREFIX)
  const data = new Uint8Array(32)
  data.set(modl, 0)
  data.set(palletId, 4)
  return data
}

export async function submitWithRetry(
  tx: any,
  signer: any,
  api: ApiPromise,
  label: string,
  maxRetries: number = 3,
  timeoutMs: number = 600_000,
): Promise<{ events: any[] }> {
  let attempt = 0

  while (attempt <= maxRetries) {
    try {
      console.log(`${label} - Attempt ${attempt + 1}/${maxRetries + 1}`)

      const result = await new Promise<{ events: any[] }>((resolve, reject) => {
        let unsubscribe: any

        const timer = setTimeout(() => {
          if (unsubscribe) unsubscribe()
          console.log(`${label} timed out after ${timeoutMs}ms`)
          reject(new Error('TIMEOUT'))
        }, timeoutMs)

        tx.signAndSend(
          signer,
          { nonce: -1, era: 0 },
          (result: ISubmittableResult) => {
            const { status, events, dispatchError } = result

            if (status.isInBlock) {
              clearTimeout(timer)
              if (unsubscribe) unsubscribe()

              console.log(
                `${label} included in block ${status.asInBlock.toHex()}`,
              )

              if (dispatchError) {
                if (dispatchError.isModule) {
                  const decoded = api.registry.findMetaError(
                    dispatchError.asModule,
                  )
                  reject(
                    new Error(
                      `${decoded.section}.${decoded.name}: ${decoded.docs.join(
                        ' ',
                      )}`,
                    ),
                  )
                } else {
                  reject(new Error(dispatchError.toString()))
                }
                return
              }

              resolve({ events: Array.from(events) })
            } else if (status.isInvalid) {
              clearTimeout(timer)
              if (unsubscribe) unsubscribe()
              console.log(`${label} marked as Invalid`)
              reject(new Error('INVALID_TX'))
            } else if (status.isDropped) {
              clearTimeout(timer)
              if (unsubscribe) unsubscribe()
              reject(new Error(`${label} dropped`))
            }
          },
        )
          .then((unsub: any) => {
            unsubscribe = unsub
            // Produce a block so the transaction gets included on the dev chain
            ;(api.rpc as any)('dev_newBlock', { count: 1 }).catch(() => {})
          })
          .catch((error: any) => {
            clearTimeout(timer)
            reject(error)
          })
      })

      return result
    } catch (error: any) {
      const msg = error.message || ''
      const isRetryable =
        msg === 'INVALID_TX' ||
        msg === 'TIMEOUT' ||
        msg.includes('1010') ||
        msg.includes('payment')
      if (isRetryable && attempt < maxRetries) {
        console.log(`Retrying ${label}...`)
        attempt++
        await new Promise((resolve) => setTimeout(resolve, 2_000))
        continue
      }
      throw error
    }
  }

  throw new Error(`${label} failed after ${maxRetries + 1} attempts`)
}

export function ethAddressFromPubKey(pubKey: string): string {
  const hash = ethers.keccak256('0x' + pubKey.slice(4))
  return '0x' + hash.slice(-40)
}

export function constructSignedTransaction(
  unsignedSerialized: string,
  signature: any,
): string {
  const tx = ethers.Transaction.from(unsignedSerialized)

  const rHex = ethers.hexlify(signature.bigR.x)
  const sHex = ethers.hexlify(signature.s)

  tx.signature = {
    r: rHex,
    s: sHex,
    v: signature.recoveryId,
  }

  return tx.serialized
}

export async function waitForReadResponse(
  api: ApiPromise,
  requestId: string,
  timeout: number,
): Promise<any> {
  return new Promise((resolve) => {
    let unsubscribe: any
    const timer = setTimeout(() => {
      if (unsubscribe) unsubscribe()
      resolve(null)
    }, timeout)

    api.query.system
      .events((events: any) => {
        events.forEach((record: any) => {
          const { event } = record
          if (
            event.section === 'signet' &&
            event.method === 'RespondBidirectionalEvent'
          ) {
            const [reqId, responder, output, signature] = event.data
            if (ethers.hexlify(reqId.toU8a()) === requestId) {
              clearTimeout(timer)
              if (unsubscribe) unsubscribe()
              resolve({
                responder: responder.toString(),
                output: Array.from(output.toU8a()),
                signature: signature.toJSON(),
              })
            }
          }
        })
      })
      .then((unsub: any) => {
        unsubscribe = unsub
      })
  })
}

export async function getTokenFree(
  api: ApiPromise,
  who: string,
  assetId: number,
): Promise<bigint> {
  const acc = await api.query.tokens.accounts(who, assetId)
  return (acc as any).free as unknown as bigint
}

export async function transferAsset(
  api: ApiPromise,
  from: any,
  to: string,
  assetId: number,
  amount: bigint | string,
  label: string,
) {
  const tx = api.tx.tokens.transfer(to, assetId, amount)
  await submitWithRetry(tx, from, api, label)
}

export async function createApi(): Promise<ApiPromise> {
  return ApiPromise.create({
    provider: new WsProvider(ENV.SUBSTRATE_WS_ENDPOINT),
    types: {
      AffinePoint: { x: '[u8; 32]', y: '[u8; 32]' },
      Signature: { big_r: 'AffinePoint', s: '[u8; 32]', recovery_id: 'u8' },
    },
  })
}

export function createKeyringAndAccounts() {
  const keyring = new Keyring({ type: 'sr25519' })
  const alice = keyring.addFromUri('//Alice')
  const bob = keyring.addFromUri('//Bob')
  return { keyring, alice, bob }
}

export async function ensureBobHasAssets(
  api: ApiPromise,
  bob: any,
  faucetAsset: number,
) {
  console.log(`Bob address: ${bob.address}`)

  const { data: bobBalance } = (await api.query.system.account(
    bob.address,
  )) as any

  const bobFaucetBalance = await getTokenFree(api, bob.address, faucetAsset)

  if (bobBalance.free.toBigInt() < MIN_BOB_NATIVE_BALANCE) {
    throw new Error(
      `Bob has insufficient native balance: ${bobBalance.free.toBigInt()}. ` +
        `Expected at least ${MIN_BOB_NATIVE_BALANCE}. Fund Bob via chopsticks config.`,
    )
  }

  if (bobFaucetBalance < ethers.parseEther('1')) {
    throw new Error(
      `Bob has insufficient faucet asset (${faucetAsset}) balance: ${bobFaucetBalance}. ` +
        `Fund Bob via chopsticks config.`,
    )
  }

  console.log(
    `Bob balances: native=${bobBalance.free.toBigInt()}, faucetAsset(${faucetAsset})=${bobFaucetBalance}`,
  )
}

export async function logAliceTokenBalances(
  api: ApiPromise,
  alice: any,
  faucetAsset: number,
  feeAsset: number,
) {
  const faucetBal = await getTokenFree(api, alice.address, faucetAsset)
  const feeBal = await getTokenFree(api, alice.address, feeAsset)

  console.log(
    'Alice balances:',
    'faucetBalance =',
    faucetBal.toString(),
    'feeBalance =',
    feeBal.toString(),
  )
}

export async function fundPalletAccounts(
  api: ApiPromise,
  alice: any,
  faucetAsset: number,
): Promise<{ palletSS58: string }> {
  const palletAccountId = getPalletAccountId()
  const palletSS58 = encodeAddress(palletAccountId, ENV.SS58_PREFIX)
  console.log(`Pallet address: ${palletSS58}`)

  // Warm up: prefetch pallet storage so chopsticks caches it before tx submission
  await api.query.system.account(palletSS58)
  await api.query.tokens.accounts(palletSS58, faucetAsset)

  await transferAsset(
    api,
    alice,
    palletSS58,
    faucetAsset,
    PALLET_FAUCET_FUND,
    `Fund pallet faucet asset ${faucetAsset}`,
  )

  const { data: palletBalance } = (await api.query.system.account(
    palletSS58,
  )) as any

  if (palletBalance.free.toBigInt() < PALLET_MIN_NATIVE_BALANCE) {
    console.log(`Funding pallet native balance ${palletSS58}...`)

    const fundTx = api.tx.balances.transferKeepAlive(
      palletSS58,
      PALLET_MIN_NATIVE_BALANCE,
    )
    await submitWithRetry(fundTx, alice, api, 'Fund pallet account')
  }

  return { palletSS58 }
}

export function deriveEthAddress(): {
  derivedPubKey: string
  derivedEthAddress: string
} {
  const derivedPubKey = KeyDerivation.derivePublicKey(
    ENV.ROOT_PUBLIC_KEY,
    ENV.SUBSTRATE_CHAIN_ID,
  )

  const derivedEthAddress = ethAddressFromPubKey(derivedPubKey)

  console.log(`\nDerived Ethereum Address: ${derivedEthAddress}`)

  return { derivedPubKey, derivedEthAddress }
}

export async function ensureDerivedEthHasGas(
  provider: ethers.JsonRpcProvider,
  derivedEthAddress: string,
) {
  const ethBalance = await provider.getBalance(derivedEthAddress)
  const feeData = await provider.getFeeData()

  const maxFeePerGas = feeData.maxFeePerGas || ENV.DEFAULT_MAX_FEE_PER_GAS
  const estimatedGas = maxFeePerGas * ENV.GAS_LIMIT

  console.log(`Balances for ${derivedEthAddress}:`)
  console.log(`   ETH: ${ethers.formatEther(ethBalance)}`)
  console.log(
    `   Estimated gas needed: ${ethers.formatEther(estimatedGas)} ETH\n`,
  )

  if (ethBalance < estimatedGas) {
    throw new Error(
      `Insufficient ETH at ${derivedEthAddress}\n` +
        `   Need: ${ethers.formatEther(estimatedGas)} ETH\n` +
        `   Have: ${ethers.formatEther(ethBalance)} ETH\n` +
        `   Please fund this address with ETH for gas`,
    )
  }
}

export async function initializeVaultIfNeeded(api: ApiPromise) {
  const cfgOpt = await (api.query as any).ethDispenser.dispenserConfig()
  const cfg = cfgOpt.toJSON() as any
  console.log('Dispenser config JSON ->', cfg)

  if (cfg?.paused === true) {
    console.log('Dispenser is paused; unpausing via Root...')
    const unpauseCall = (api.tx as any).ethDispenser.unpause()
    await executeAsRootViaScheduler(
      api,
      unpauseCall,
      'Unpause ethDispenser via Root',
    )
  }

  const current = (
    await (api.query as any).ethDispenser.faucetBalanceWei()
  ).toBigInt()
  const threshold = (
    (api.consts as any).ethDispenser.minFaucetEthThreshold as any
  ).toBigInt()

  console.log('Current faucetBalanceWei =', current.toString())
  console.log('MinFaucetEthThreshold =', threshold.toString())

  const targetMin = threshold + ENV.REQUEST_FUND_AMOUNT + ethers.parseEther('1')
  if (current >= targetMin) {
    console.log('FaucetBalanceWei already sufficient, skipping top-up')
    return
  }

  const addWei = targetMin - current
  console.log('Topping up faucet balance via Root, add =', addWei.toString())

  const setBalCall = (api.tx as any).ethDispenser.setFaucetBalance(
    addWei.toString(),
  )
  await executeAsRootViaScheduler(
    api,
    setBalCall,
    'Top up ethDispenser faucet balance via Root',
  )

  const after = await (api.query as any).ethDispenser.faucetBalanceWei()
  console.log('faucetBalanceWei after =', after.toString())
}

// ---------------------------------------------------------------------------
// Root execution helpers
// ---------------------------------------------------------------------------

export async function executeAsRootViaReferendum(
  api: ApiPromise,
  signer: any,
  call: any,
  label: string,
  maxRetries = 1,
  timeoutMs = 300_000,
): Promise<number> {
  console.log(`\n=== ${label}: starting Root execution via Referenda ===`)

  const encodedCall = call.method.toHex()
  const encodedHash = blake2AsHex(encodedCall)

  console.log(`${label}: noting preimage...`)
  const notePreimageTx = api.tx.preimage.notePreimage(encodedCall)
  await submitWithRetry(
    notePreimageTx,
    signer,
    api,
    `${label} - notePreimage`,
    maxRetries,
    timeoutMs,
  )

  console.log(`${label}: submitting referendum with Root origin...`)
  const proposalOrigin = { system: 'Root' }
  const proposalCall = {
    Lookup: {
      hash: encodedHash,
      len: encodedCall.length / 2 - 1,
    },
  }
  const enactmentMoment = { After: 1 }

  const submitTx = api.tx.referenda.submit(
    proposalOrigin,
    proposalCall,
    enactmentMoment,
  )

  await submitWithRetry(
    submitTx,
    signer,
    api,
    `${label} - submitReferendum`,
    maxRetries,
    timeoutMs,
  )

  const referendumIndex =
    parseInt((await api.query.referenda.referendumCount()).toString()) - 1
  console.log(`${label}: referendumIndex = ${referendumIndex}`)

  const faucetAsset = (api.consts.ethDispenser.faucetAsset as any).toNumber()

  let { data } = (await api.query.system.account(signer.address)) as any
  console.log('signer free balance =', data.free.toBigInt().toString())
  const acc = (await api.query.tokens.accounts(
    signer.address,
    faucetAsset,
  )) as any
  console.log(
    'faucet asset id =',
    faucetAsset,
    'faucet asset free =',
    acc.free.toString(),
  )

  const tracks: any = api.consts.referenda.tracks
  console.log('Tracks:', tracks.toHuman())

  console.log(`${label}: placing decision deposit...`)
  const decisionDepositTx =
    api.tx.referenda.placeDecisionDeposit(referendumIndex)
  await submitWithRetry(
    decisionDepositTx,
    signer,
    api,
    `${label} - decisionDeposit`,
    maxRetries,
    timeoutMs,
  )

  console.log(`${label}: voting AYE on referendum...`)
  data = ((await api.query.system.account(signer.address)) as any).data
  const free = data.free.toBigInt()

  const voteAmount = (free * 5n) / 10n

  console.log(
    `${label}: free balance = ${free.toString()}, voteAmount = ${voteAmount.toString()}`,
  )

  const voteTx = api.tx.convictionVoting.vote(referendumIndex, {
    Standard: {
      balance: voteAmount,
      vote: { aye: true, conviction: 'Locked1x' },
    },
  })

  await submitWithRetry(
    voteTx,
    signer,
    api,
    `${label} - vote`,
    maxRetries,
    timeoutMs,
  )

  console.log(`${label}: waiting for referendum to progress...`)

  await (api.rpc as any)('dev_newBlock', { count: 10 })
  const info = await api.query.referenda.referendumInfoFor(referendumIndex)
  console.log('Referendum info:', info.toHuman())

  console.log(
    `=== ${label}: Root call scheduled via referenda (index ${referendumIndex}) ===\n`,
  )

  return referendumIndex
}

export async function executeAsRootViaScheduler(
  api: ApiPromise,
  call: SubmittableExtrinsic<'promise'>,
  label: string,
) {
  const header = await api.rpc.chain.getHeader()
  const number = header.number.toNumber()
  const callHex = call.method.toHex()

  console.log(`${label}: scheduling as Root in block ${number + 1}`)

  await (api.rpc as any)('dev_setStorage', {
    scheduler: {
      agenda: [
        [
          [number + 1],
          [
            {
              call: { Inline: callHex },
              origin: { system: 'Root' },
            },
          ],
        ],
      ],
    },
  })

  await (api.rpc as any)('dev_newBlock', { count: 1 })

  console.log(`${label}: executed in new block`)
}
