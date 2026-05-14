import { ApiPromise, WsProvider, Keyring } from '@polkadot/api'
import { ISubmittableResult } from '@polkadot/types/types'
import { encodeAddress } from '@polkadot/keyring'
import { u8aToHex } from '@polkadot/util'
import { ethers } from 'ethers'
import { KeyDerivation } from './key-derivation'
import { blake2AsHex } from '@polkadot/util-crypto'
import { SubmittableExtrinsic } from '@polkadot/api/types'
import { ENV } from './env'

// --- Substrate funding thresholds (not network-specific) ---
export const MIN_BOB_NATIVE_BALANCE = 1
export const PALLET_MIN_NATIVE_BALANCE = 10_000_000_000_000n
export const BOB_NATIVE_TOPUP = 100_000_000_000_000n
// Minimum HDX Bob needs to send substrate txs as the MPC server signer
export const SERVER_SIGNER_MIN_BALANCE = 50_000_000_000_000n // 50 HDX
export const SERVER_SIGNER_TOPUP = 200_000_000_000_000n // 200 HDX
export const PALLET_FAUCET_FUND = ethers.parseEther('100')

export const PALLET_ID_STR = 'py/fucet'
export const MODL_PREFIX = 'modl'

// Fixed signing path used by the dispenser pallet for all users
export const DISPENSER_SIGNING_PATH = 'dispenser'

// Cached result for dev chain detection
let _isDevChain: boolean | null = null

/**
 * Probe whether the connected node supports dev RPCs (chopsticks).
 * Tries multiple detection methods. Result is cached for the lifetime of the process.
 */
export async function isDevChain(api: ApiPromise): Promise<boolean> {
  if (_isDevChain !== null) return _isDevChain
  // Try dev_setBlockBuildMode which doesn't produce blocks — safest probe
  for (const method of ['dev_setBlockBuildMode', 'dev_newBlock']) {
    try {
      if (method === 'dev_setBlockBuildMode') {
        await (api.rpc as any)(method, 'Instant')
      } else {
        await (api.rpc as any)(method, { count: 1 })
      }
      _isDevChain = true
      console.log(`Dev chain detected via ${method}`)
      return true
    } catch {}
  }
  _isDevChain = false
  console.log('Not a dev chain — will use governance for Root calls')
  return false
}

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

// Large tip so our transactions always replace stuck pool entries on live chains.
const DEFAULT_TIP = 100_000_000_000_000n // 100 HDX

export async function submitWithRetry(
  tx: any,
  signer: any,
  api: ApiPromise,
  label: string,
  maxRetries: number = 3,
  timeoutMs: number = 600_000,
): Promise<{ events: any[] }> {
  let attempt = 0
  const dev = await isDevChain(api)

  while (attempt <= maxRetries) {
    const tip = dev ? 0n : DEFAULT_TIP * BigInt(2 ** attempt)

    try {
      console.log(
        `${label} - Attempt ${attempt + 1}/${maxRetries + 1}${tip > 0n ? ` (tip: ${tip})` : ''}`,
      )

      // Capture nonce before submission so we can poll for inclusion as a fallback
      const preNonce = dev
        ? 0
        : (await api.rpc.system.accountNextIndex(signer.address)).toNumber()

      const result = await new Promise<{ events: any[] }>((resolve, reject) => {
        let unsubscribe: any
        let resolved = false
        let pollTimer: any

        const cleanup = () => {
          resolved = true
          if (unsubscribe) unsubscribe()
          if (pollTimer) clearInterval(pollTimer)
        }

        const timer = setTimeout(() => {
          cleanup()
          console.log(`${label} timed out after ${timeoutMs}ms`)
          reject(new Error('TIMEOUT'))
        }, timeoutMs)

        const clearAll = () => {
          clearTimeout(timer)
          cleanup()
        }

        // Polling fallback: if WS subscription misses InBlock, detect via nonce advance
        const startPollFallback = () => {
          if (pollTimer || dev) return
          pollTimer = setInterval(async () => {
            if (resolved) return
            try {
              const currentNonce = (
                await api.rpc.system.accountNextIndex(signer.address)
              ).toNumber()
              if (currentNonce > preNonce) {
                console.log(
                  `${label} nonce advanced ${preNonce} → ${currentNonce} (poll fallback)`,
                )
                clearAll()

                // Scan recent blocks to find the actual events for our tx
                const blockEvents = await scanRecentBlocksForExtrinsicEvents(
                  api,
                  signer.address,
                  preNonce,
                )

                if (blockEvents) {
                  // Check for dispatch errors in the recovered events
                  const dispatchError = blockEvents.find(
                    (r: any) =>
                      r.event.section === 'system' &&
                      r.event.method === 'ExtrinsicFailed',
                  )
                  if (dispatchError) {
                    const errorData = dispatchError.event.data[0]
                    if (errorData?.isModule) {
                      const decoded = api.registry.findMetaError(errorData.asModule)
                      reject(
                        new Error(
                          `${decoded.section}.${decoded.name}: ${decoded.docs.join(' ')}`,
                        ),
                      )
                    } else {
                      reject(new Error(`Dispatch error: ${errorData?.toString()}`))
                    }
                    return
                  }
                  console.log(
                    `${label} recovered ${blockEvents.length} events from block scan`,
                  )
                  resolve({ events: blockEvents })
                } else {
                  console.log(`${label} could not recover events from block scan`)
                  resolve({ events: [] })
                }
              }
            } catch {}
          }, 6_000) // check every ~1 block
        }

        const signingOpts: any = dev
          ? { nonce: -1, era: 0 }
          : { nonce: -1, tip: tip.toString() }

        tx.signAndSend(
          signer,
          signingOpts,
          (result: ISubmittableResult) => {
            if (resolved) return
            const { status, events, dispatchError } = result

            // Log every status update so we can diagnose hangs
            console.log(`${label} status: ${status.type}`)

            if (status.isReady || status.type === 'Ready') {
              // Start polling fallback 15s after Ready (live chains only)
              setTimeout(() => startPollFallback(), 15_000)
            }

            if (status.isInBlock) {
              clearAll()

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
              clearAll()
              console.log(`${label} marked as Invalid`)
              reject(new Error('INVALID_TX'))
            } else if (status.isDropped) {
              clearAll()
              reject(new Error(`${label} dropped`))
            }
          },
        )
          .then((unsub: any) => {
            unsubscribe = unsub
            // Produce a block so the transaction gets included on the dev chain
            if (dev) {
              ;(api.rpc as any)('dev_newBlock', { count: 1 }).catch(() => {})
            }
          })
          .catch((error: any) => {
            clearAll()
            reject(error)
          })
      })

      return result
    } catch (error: any) {
      const msg = String(error?.message || error)
      const isRetryable =
        msg === 'INVALID_TX' ||
        msg === 'TIMEOUT' ||
        msg.includes('1010') ||
        msg.includes('1014') ||
        msg.includes('Priority is too low') ||
        msg.includes('payment')
      if (isRetryable && attempt < maxRetries) {
        console.log(`Retrying ${label} (will bump tip)...`)
        attempt++
        await new Promise((resolve) => setTimeout(resolve, 2_000))
        continue
      }
      throw error
    }
  }

  throw new Error(`${label} failed after ${maxRetries + 1} attempts`)
}

/**
 * When the nonce polling fallback fires, scan recent blocks to find events
 * for the extrinsic sent by `sender` with the given `nonce`.
 */
async function scanRecentBlocksForExtrinsicEvents(
  api: ApiPromise,
  sender: string,
  nonce: number,
): Promise<any[] | null> {
  try {
    // Decode sender to raw account ID for reliable comparison
    // (SS58 prefix may differ between keyring and chain)
    const keyring = new Keyring()
    const senderAccountId = u8aToHex(keyring.decodeAddress(sender))

    const header = await api.rpc.chain.getHeader()
    const currentBlock = header.number.toNumber()
    const startBlock = Math.max(1, currentBlock - 10) // check last 10 blocks

    for (let i = currentBlock; i >= startBlock; i--) {
      const hash = await api.rpc.chain.getBlockHash(i)
      const signedBlock = await api.rpc.chain.getBlock(hash)
      const allEvents = await api.query.system.events.at(hash) as any

      // Find our extrinsic by matching sender account ID and nonce
      const extrinsics = signedBlock.block.extrinsics
      for (let extIdx = 0; extIdx < extrinsics.length; extIdx++) {
        const ext = extrinsics[extIdx]
        if (!ext.isSigned) continue

        const extSignerHex = u8aToHex(keyring.decodeAddress(ext.signer.toString()))
        const extNonce = ext.nonce.toNumber()

        if (extSignerHex === senderAccountId && extNonce === nonce) {
          // Found our extrinsic — collect its events
          const events = allEvents.filter(
            (record: any) =>
              record.phase.isApplyExtrinsic &&
              record.phase.asApplyExtrinsic.toNumber() === extIdx,
          )
          console.log(
            `Found extrinsic in block ${i} (idx ${extIdx}) with ${events.length} events`,
          )
          return Array.from(events)
        }
      }
    }
    console.log(
      `Could not find extrinsic (sender=${sender}, nonce=${nonce}) in blocks ${startBlock}..${currentBlock}`,
    )
  } catch (err: any) {
    console.warn(`Failed to scan blocks for extrinsic events: ${err.message}`)
  }
  return null
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
    let resolved = false

    const timer = setTimeout(() => {
      resolved = true
      if (unsubscribe) unsubscribe()
      resolve(null)
    }, timeout)

    const done = (result: any) => {
      if (resolved) return
      resolved = true
      clearTimeout(timer)
      if (unsubscribe) unsubscribe()
      resolve(result)
    }

    const matchReadEvent = (event: any): any => {
      if (
        event.section === 'signet' &&
        event.method === 'RespondBidirectionalEvent'
      ) {
        const [reqId, responder, output, signature] = event.data
        if (ethers.hexlify(reqId.toU8a()) === requestId) {
          return {
            responder: responder.toString(),
            output: Array.from(output.toU8a()),
            signature: signature.toJSON(),
          }
        }
      }
      return null
    }

    // 1. Subscribe to new events
    api.query.system
      .events((events: any) => {
        events.forEach((record: any) => {
          const result = matchReadEvent(record.event)
          if (result) done(result)
        })
      })
      .then((unsub: any) => {
        unsubscribe = unsub
      })

    // 2. Scan recent blocks for already-emitted events
    scanRecentBlocksForReadResponse(api, requestId, 30)
      .then((result) => {
        if (result) done(result)
      })
      .catch(() => {})
  })
}

async function scanRecentBlocksForReadResponse(
  api: ApiPromise,
  requestId: string,
  numBlocks: number,
): Promise<any> {
  try {
    const header = await api.rpc.chain.getHeader()
    const currentBlock = header.number.toNumber()
    const startBlock = Math.max(1, currentBlock - numBlocks)

    console.log(`Scanning blocks ${startBlock}..${currentBlock} for RespondBidirectionalEvent...`)

    for (let i = currentBlock; i >= startBlock; i--) {
      const hash = await api.rpc.chain.getBlockHash(i)
      const events = await api.query.system.events.at(hash) as any

      for (const record of events) {
        const { event } = record
        if (
          event.section === 'signet' &&
          event.method === 'RespondBidirectionalEvent'
        ) {
          const [reqId, responder, output, signature] = event.data
          if (ethers.hexlify(reqId.toU8a()) === requestId) {
            console.log(`Found RespondBidirectionalEvent in block ${i}`)
            return {
              responder: responder.toString(),
              output: Array.from(output.toU8a()),
              signature: signature.toJSON(),
            }
          }
        }
      }
    }
  } catch (err) {
    console.warn('Failed to scan recent blocks for RespondBidirectionalEvent:', err)
  }
  return null
}

export async function getTokenFree(
  api: ApiPromise,
  who: string,
  assetId: number,
): Promise<bigint> {
  const acc = await api.query.tokens.accounts(who, assetId)
  const free = (acc as any).free
  // Handle both codec types (.toBigInt()) and raw bigints
  return typeof free === 'bigint' ? free : BigInt(free.toString())
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
    console.warn(
      `[WARN] Bob has insufficient native balance: ${bobBalance.free.toBigInt()}. ` +
        `Expected at least ${MIN_BOB_NATIVE_BALANCE}. Skipping Bob check (not required for test flow).`,
    )
    return
  }

  if (bobFaucetBalance < ethers.parseEther('1')) {
    console.warn(
      `[WARN] Bob has insufficient faucet asset (${faucetAsset}) balance: ${bobFaucetBalance}. ` +
        `Skipping Bob check (not required for test flow).`,
    )
    return
  }

  console.log(
    `Bob balances: native=${bobBalance.free.toBigInt()}, faucetAsset(${faucetAsset})=${bobFaucetBalance}`,
  )
}

/**
 * Ensure Bob (the MPC response server signer) has enough native HDX to pay
 * substrate transaction fees. Alice transfers HDX to Bob if needed.
 */
export async function ensureServerSignerFunded(
  api: ApiPromise,
  alice: any,
  bob: any,
) {
  const { data: bobBalance } = (await api.query.system.account(bob.address)) as any
  const bobFree = bobBalance.free.toBigInt()
  console.log(`Server signer (Bob) native balance: ${bobFree}`)

  if (bobFree >= SERVER_SIGNER_MIN_BALANCE) {
    console.log('Server signer has sufficient balance')
    return
  }

  console.log(`Funding server signer (Bob) with ${SERVER_SIGNER_TOPUP} HDX...`)
  const fundTx = api.tx.balances.transferKeepAlive(bob.address, SERVER_SIGNER_TOPUP.toString())
  await submitWithRetry(fundTx, alice, api, 'Fund server signer (Bob)')

  const { data: afterBalance } = (await api.query.system.account(bob.address)) as any
  console.log(`Server signer (Bob) balance after funding: ${afterBalance.free.toBigInt()}`)
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
): Promise<{ palletSS58: string; palletSS58Prefix0: string }> {
  const palletAccountId = getPalletAccountId()
  const palletSS58 = encodeAddress(palletAccountId, ENV.SS58_PREFIX)
  // The pallet always uses SS58 prefix 0 for request ID computation
  const palletSS58Prefix0 = encodeAddress(palletAccountId, 0)
  console.log(`Pallet address: ${palletSS58}`)
  console.log(`Pallet address (prefix 0, for request ID): ${palletSS58Prefix0}`)

  // Warm up: prefetch pallet storage so chopsticks caches it before tx submission
  await api.query.system.account(palletSS58)
  await api.query.tokens.accounts(palletSS58, faucetAsset)

  // The dispenser pallet collects WETH collateral from the REQUESTER (Alice),
  // not from the pallet account. Ensure Alice has enough WETH.
  await ensureAliceHasFaucetAsset(api, alice, faucetAsset)

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
  } else {
    console.log(`Pallet native balance sufficient (${palletBalance.free.toBigInt()}), skipping`)
  }

  // Fund the signet pallet account so it can receive signature deposits.
  // sign_bidirectional transfers signature_deposit from the dispenser pallet
  // to the signet pallet; if the signet pallet has 0 HDX the transfer fails
  // with BelowMinimum because the deposit may be below the native ED.
  await fundSignetPalletAccount(api, alice)

  return { palletSS58, palletSS58Prefix0 }
}

const SIGNET_PALLET_ID_STR = 'py/signt'

async function fundSignetPalletAccount(api: ApiPromise, alice: any) {
  const modl = new TextEncoder().encode(MODL_PREFIX)
  const palletId = new TextEncoder().encode(SIGNET_PALLET_ID_STR)
  const data = new Uint8Array(32)
  data.set(modl, 0)
  data.set(palletId, 4)
  const signetSS58 = encodeAddress(data, ENV.SS58_PREFIX)

  const { data: bal } = (await api.query.system.account(signetSS58)) as any
  const free = bal.free.toBigInt()
  console.log(`Signet pallet (${signetSS58}) native balance: ${free}`)

  if (free >= PALLET_MIN_NATIVE_BALANCE) {
    console.log('Signet pallet balance sufficient, skipping')
    return
  }

  console.log(`Funding signet pallet account...`)
  const fundTx = api.tx.balances.transferKeepAlive(
    signetSS58,
    PALLET_MIN_NATIVE_BALANCE,
  )
  await submitWithRetry(fundTx, alice, api, 'Fund signet pallet account')
}

/**
 * Ensure Alice has enough faucet asset (WETH) to pay collateral in requestFund.
 * On dev chains, Alice already has tokens from the forked state.
 * On live chains (lark), we mint via currencies.updateBalance through Root governance.
 */
async function ensureAliceHasFaucetAsset(
  api: ApiPromise,
  alice: any,
  faucetAsset: number,
) {
  const aliceBal = await getTokenFree(api, alice.address, faucetAsset)
  // Need enough for at least a few requestFund calls
  const needed = ENV.REQUEST_FUND_AMOUNT * 10n
  console.log(
    `Alice faucet asset (${faucetAsset}) balance: ${aliceBal}, needed: ${needed}`,
  )

  if (aliceBal >= needed) {
    console.log('Alice has sufficient faucet asset balance')
    return
  }

  if (await isDevChain(api)) {
    // On dev chains, Alice should already have balance from fork state
    console.warn(
      `[WARN] Alice has insufficient faucet asset (${faucetAsset}) on dev chain. ` +
        `Check chopsticks fork config.`,
    )
    return
  }

  const mintAmount = needed - aliceBal
  console.log(
    `Minting ${mintAmount} of asset ${faucetAsset} to Alice via Root governance...`,
  )

  const mintCall = (api.tx as any).currencies.updateBalance(
    alice.address,
    faucetAsset,
    mintAmount.toString(),
  )

  await executeAsRoot(
    api,
    alice,
    mintCall,
    `Mint faucet asset ${faucetAsset} to Alice`,
  )

  const afterBal = await getTokenFree(api, alice.address, faucetAsset)
  console.log(`Alice faucet asset (${faucetAsset}) balance after mint: ${afterBal}`)
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

  if (ethBalance >= estimatedGas) return

  if (ENV.EVM_NETWORK === 'anvil') {
    const ANVIL_DEFAULT_KEY = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80'
    const funder = new ethers.Wallet(ANVIL_DEFAULT_KEY, provider)
    const fundAmount = estimatedGas * 10n
    console.log(`Funding ${derivedEthAddress} with ${ethers.formatEther(fundAmount)} ETH from Anvil account...`)
    const tx = await funder.sendTransaction({ to: derivedEthAddress, value: fundAmount })
    await tx.wait()
    console.log(`Funded. Tx: ${tx.hash}`)
    return
  }

  throw new Error(
    `Insufficient ETH at ${derivedEthAddress}\n` +
      `   Need: ${ethers.formatEther(estimatedGas)} ETH\n` +
      `   Have: ${ethers.formatEther(ethBalance)} ETH\n` +
      `   Please fund this address with ETH for gas`,
  )
}

/**
 * Ensure the faucet contract's MPC address is set to the derived address.
 * On local Anvil, the deployer (account 0) is the owner and can call setMPC.
 */
export async function ensureFaucetMpcAddress(
  provider: ethers.JsonRpcProvider,
  derivedEthAddress: string,
) {
  const faucetContract = new ethers.Contract(
    ENV.FAUCET_ADDRESS,
    ['function mpc() view returns (address)', 'function setMPC(address)'],
    provider,
  )

  const currentMpc = await faucetContract.mpc()
  console.log(`Faucet MPC address: ${currentMpc}`)

  if (currentMpc.toLowerCase() === derivedEthAddress.toLowerCase()) {
    console.log('Faucet MPC already set to derived address')
    return
  }

  console.log(`Setting faucet MPC to derived address ${derivedEthAddress}...`)
  // Use Anvil account 0 (deployer/owner) to call setMPC
  const ownerKey = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80'
  const ownerWallet = new ethers.Wallet(ownerKey, provider)
  const contractWithSigner = faucetContract.connect(ownerWallet)
  const tx = await (contractWithSigner as any).setMPC(derivedEthAddress)
  await tx.wait()
  console.log('Faucet MPC address updated')
}

/**
 * Ensure the dispenser pallet is configured and not paused.
 * Uses setConfig (new API — all params in one config struct, no separate faucetBalanceWei storage).
 */
export async function initializeVaultIfNeeded(api: ApiPromise, signer?: any) {
  const cfgOpt = await (api.query as any).ethDispenser.dispenserConfig()
  const cfg = cfgOpt.toJSON() as any
  console.log('Dispenser config JSON ->', cfg)

  if (!cfg) {
    console.log('Dispenser not configured; setting config via Root...')
    const setConfigCall = (api.tx as any).ethDispenser.setConfig(
      ENV.FAUCET_ADDRESS,                         // faucet_address
      ethers.parseEther('0.05').toString(),        // min_faucet_threshold (0.05 ETH)
      '0',                                        // min_request
      ethers.parseEther('1').toString(),           // max_dispense (1 ETH)
      '1000000000000',                              // dispenser_fee (1 HDX, 12 decimals)
      ethers.parseEther('10').toString(),          // faucet_balance_wei (10 ETH)
    )
    if (signer) {
      await executeAsRoot(api, signer, setConfigCall, 'Set ethDispenser config via Root')
    } else {
      await executeAsRootViaScheduler(api, setConfigCall, 'Set ethDispenser config via Root')
    }
    return
  }

  if (cfg.paused === true) {
    console.log('Dispenser is paused; unpausing via Root...')
    const unpauseCall = (api.tx as any).ethDispenser.unpause()
    if (signer) {
      await executeAsRoot(api, signer, unpauseCall, 'Unpause ethDispenser via Root')
    } else {
      await executeAsRootViaScheduler(api, unpauseCall, 'Unpause ethDispenser via Root')
    }
  } else {
    console.log('Dispenser is not paused, skipping unpause')
  }

  // Check if faucet balance is sufficient
  const currentBalance = BigInt(cfg.faucetBalanceWei || '0')
  const threshold = BigInt(cfg.minFaucetThreshold || '0')

  console.log('Current faucetBalanceWei =', currentBalance.toString())
  console.log('minFaucetThreshold =', threshold.toString())

  const targetMin = threshold + ENV.REQUEST_FUND_AMOUNT + ethers.parseEther('1')
  if (currentBalance >= targetMin) {
    console.log('FaucetBalanceWei already sufficient, skipping reconfigure')
    return
  }

  console.log('Reconfiguring dispenser with higher faucet balance via Root...')
  const setConfigCall = (api.tx as any).ethDispenser.setConfig(
    cfg.faucetAddress,
    cfg.minFaucetThreshold.toString(),
    cfg.minRequest.toString(),
    cfg.maxDispense.toString(),
    cfg.dispenserFee.toString(),
    targetMin.toString(),
  )
  if (signer) {
    await executeAsRoot(api, signer, setConfigCall, 'Update ethDispenser faucet balance via Root')
  } else {
    await executeAsRootViaScheduler(api, setConfigCall, 'Update ethDispenser faucet balance via Root')
  }

  const afterOpt = await (api.query as any).ethDispenser.dispenserConfig()
  const afterCfg = afterOpt.toJSON() as any
  console.log('faucetBalanceWei after =', afterCfg?.faucetBalanceWei)
}

// ---------------------------------------------------------------------------
// Root execution helpers
// ---------------------------------------------------------------------------

/**
 * Execute a call with elevated origin, auto-detecting the best strategy:
 *   1. Chopsticks → dev_setStorage scheduler (instant)
 *   2. Signer is TC member → technicalCommittee.propose (fast)
 *   3. Fallback → referendum (slow, but always works)
 */
export async function executeAsRoot(
  api: ApiPromise,
  signer: any,
  call: SubmittableExtrinsic<'promise'>,
  label: string,
) {
  if (await isDevChain(api)) {
    await executeAsRootViaScheduler(api, call, label)
    return
  }

  // Try TC path first — much faster than governance referendum
  const isTcMember = await isSignerTcMember(api, signer)
  if (isTcMember) {
    await executeViaTechCommittee(api, signer, call, label)
    return
  }

  console.log(`${label}: signer is not a TC member, falling back to referendum`)
  await executeAsRootViaReferendum(api, signer, call, label)
}

/**
 * Remove votes on completed/non-ongoing referendums to free up voting slots.
 * MaxVotes=25 on Hydration — Alice can't vote on new referendums if she's hit the limit.
 */
async function cleanupOldVotes(
  api: ApiPromise,
  signer: any,
  trackId: number,
  label: string,
) {
  try {
    const votingInfo = await (api.query as any).convictionVoting.votingFor(
      signer.address,
      trackId,
    )
    const voting = votingInfo.toJSON() as any

    if (!voting?.casting?.votes) return

    const votes = voting.casting.votes as [number, any][]
    console.log(`${label}: found ${votes.length} existing votes on track ${trackId}`)

    if (votes.length < 10) {
      console.log(`${label}: vote count under limit, no cleanup needed`)
      return
    }

    // Find votes for non-ongoing referendums
    const toRemove: number[] = []
    for (const [refIndex] of votes) {
      const info = await api.query.referenda.referendumInfoFor(refIndex)
      const human = info.toHuman() as any
      if (!human?.Ongoing) {
        toRemove.push(refIndex)
      }
    }

    if (toRemove.length === 0) {
      console.log(`${label}: all votes are for ongoing referendums, nothing to clean up`)
      return
    }

    console.log(`${label}: removing ${toRemove.length} old votes: [${toRemove.join(', ')}]`)

    // Batch remove votes
    const removeCalls = toRemove.map((refIndex) =>
      api.tx.convictionVoting.removeVote(trackId, refIndex),
    )

    const batchTx = api.tx.utility.batchAll(removeCalls)
    await submitWithRetry(batchTx, signer, api, `${label} - cleanup old votes`)

    console.log(`${label}: cleaned up ${toRemove.length} old votes`)
  } catch (err: any) {
    console.warn(`${label}: failed to cleanup old votes: ${err.message || err}`)
  }
}

async function isSignerTcMember(api: ApiPromise, signer: any): Promise<boolean> {
  try {
    const members = await (api.query as any).technicalCommittee.members()
    const memberList = members.toJSON() as string[]
    const keyring = new Keyring()
    const signerAccountId = u8aToHex(keyring.decodeAddress(signer.address))
    return memberList.some(
      (m) => u8aToHex(keyring.decodeAddress(m)) === signerAccountId,
    )
  } catch {
    return false
  }
}

/**
 * Execute a call via Tech Committee proposal.
 * If signer is the only TC member, threshold=1 → executes immediately.
 * Otherwise creates a proposal that other TC members must vote on.
 */
async function executeViaTechCommittee(
  api: ApiPromise,
  signer: any,
  call: SubmittableExtrinsic<'promise'>,
  label: string,
) {
  const members = await (api.query as any).technicalCommittee.members()
  const memberList = members.toJSON() as string[]
  const memberCount = memberList.length
  // Majority threshold: floor(n/2) + 1
  const threshold = Math.floor(memberCount / 2) + 1

  console.log(`${label}: executing via TC (members: ${memberCount}, threshold: ${threshold})`)

  const lengthBound = call.method.encodedLength + 100
  const proposeTx = (api.tx as any).technicalCommittee.propose(
    threshold,
    call,
    lengthBound,
  )

  const result = await submitWithRetry(proposeTx, signer, api, `${label} - TC propose`)

  // If threshold=1, the call executed inline. Otherwise log the proposal index.
  if (threshold > 1) {
    for (const { event } of result.events) {
      if (event.section === 'technicalCommittee' && event.method === 'Proposed') {
        const proposalIndex = event.data[1]?.toString() ?? event.data[2]?.toString()
        console.log(`${label}: TC proposal #${proposalIndex} created. Other TC members must vote Aye.`)
      }
    }
  }

  // Check if the call was executed (Executed event = threshold was 1 or auto-closed)
  const executed = result.events.some(
    (r: any) =>
      r.event.section === 'technicalCommittee' &&
      (r.event.method === 'Executed' || r.event.method === 'Closed'),
  )

  if (executed) {
    console.log(`${label}: TC proposal executed immediately`)
  } else if (threshold > 1) {
    console.log(`${label}: TC proposal needs ${threshold - 1} more Aye votes from other members`)
    // Poll for execution (other TC members may vote via separate process)
    await pollForTcExecution(api, result, label)
  }
}

/**
 * Poll for TC proposal execution after other members vote.
 */
async function pollForTcExecution(
  api: ApiPromise,
  proposeResult: { events: any[] },
  label: string,
  timeoutMs = 600_000,
) {
  // Extract proposal hash from Proposed event
  let proposalHash: string | null = null
  for (const { event } of proposeResult.events) {
    if (event.section === 'technicalCommittee' && event.method === 'Proposed') {
      proposalHash = event.data[2]?.toHex?.() ?? event.data[2]?.toString()
    }
  }

  if (!proposalHash) {
    console.log(`${label}: could not find proposal hash, skipping poll`)
    return
  }

  const start = Date.now()
  while (Date.now() - start < timeoutMs) {
    const proposal = await (api.query as any).technicalCommittee.proposalOf(proposalHash)
    if (proposal.isNone) {
      console.log(`${label}: TC proposal executed (no longer in storage)`)
      return
    }
    console.log(`${label}: waiting for TC proposal to be voted on...`)
    await new Promise((r) => setTimeout(r, 6_000))
  }
  console.warn(`${label}: TC proposal poll timed out after ${timeoutMs}ms`)
}

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

  // Note preimage — skip if already noted
  console.log(`${label}: noting preimage...`)
  try {
    const notePreimageTx = api.tx.preimage.notePreimage(encodedCall)
    await submitWithRetry(
      notePreimageTx,
      signer,
      api,
      `${label} - notePreimage`,
      maxRetries,
      timeoutMs,
    )
  } catch (err: any) {
    if (String(err).includes('AlreadyNoted')) {
      console.log(`${label}: preimage already noted, skipping`)
    } else {
      throw err
    }
  }

  // Check if there's already an ongoing referendum for the same proposal
  let referendumIndex = -1
  const refCount = parseInt((await api.query.referenda.referendumCount()).toString())
  for (let i = refCount - 1; i >= Math.max(0, refCount - 50); i--) {
    const info = await api.query.referenda.referendumInfoFor(i)
    const human = info.toHuman() as any
    if (human?.Ongoing?.proposal?.Lookup?.hash_ === encodedHash) {
      referendumIndex = i
      console.log(`${label}: found existing ongoing referendum ${i} for this proposal, reusing`)
      break
    }
  }

  if (referendumIndex < 0) {
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

    referendumIndex =
      parseInt((await api.query.referenda.referendumCount()).toString()) - 1
  }

  console.log(`${label}: referendumIndex = ${referendumIndex}`)

  // If referendum is already completed, skip all remaining steps
  const earlyInfo = await api.query.referenda.referendumInfoFor(referendumIndex)
  const earlyHuman = earlyInfo.toHuman() as any
  if (earlyHuman?.Approved || earlyHuman?.Confirmed || earlyHuman?.Executed) {
    console.log(`${label}: referendum ${referendumIndex} already completed (${Object.keys(earlyHuman)[0]}), skipping`)
    return referendumIndex
  }
  if (earlyHuman?.Rejected || earlyHuman?.Cancelled || earlyHuman?.TimedOut || earlyHuman?.Killed) {
    console.warn(`${label}: referendum ${referendumIndex} is in terminal state: ${Object.keys(earlyHuman)[0]}. Will create a new one on next run.`)
  }

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

  // Check if decision deposit is already placed
  const refInfoBefore = await api.query.referenda.referendumInfoFor(referendumIndex)
  const refHumanBefore = refInfoBefore.toHuman() as any
  const hasDecisionDeposit = !!refHumanBefore?.Ongoing?.decisionDeposit

  if (hasDecisionDeposit) {
    console.log(`${label}: decision deposit already placed, skipping`)
  } else {
    console.log(`${label}: placing decision deposit...`)
    try {
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
    } catch (err: any) {
      if (String(err).includes('HasDeposit')) {
        console.log(`${label}: decision deposit already placed, skipping`)
      } else {
        throw err
      }
    }
  }

  // Clean up old votes to stay under MaxVotes (25) limit
  await cleanupOldVotes(api, signer, 0, label)

  // Check if already voted on this referendum
  const refInfoForVote = await api.query.referenda.referendumInfoFor(referendumIndex)
  const refHumanForVote = refInfoForVote.toHuman() as any
  const currentTally = refHumanForVote?.Ongoing?.tally
  const alreadyVoted = currentTally && currentTally.ayes !== '0'

  if (alreadyVoted) {
    console.log(`${label}: already voted (ayes=${currentTally.ayes}), skipping vote`)
  } else {
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

    try {
      await submitWithRetry(
        voteTx,
        signer,
        api,
        `${label} - vote`,
        maxRetries,
        timeoutMs,
      )
    } catch (err: any) {
      // MaxVotesReached or other vote errors — log and continue
      console.warn(`${label}: vote failed: ${err.message || err}`)
    }

    // Verify vote was counted
    const postVoteInfo = await api.query.referenda.referendumInfoFor(referendumIndex)
    const postVoteHuman = postVoteInfo.toHuman() as any
    const tally = postVoteHuman?.Ongoing?.tally
    if (tally && tally.ayes === '0') {
      console.warn(`[WARN] Vote may not have been counted (tally ayes=0). Check MaxVotes limit.`)
    } else {
      console.log(`${label}: vote confirmed, tally:`, JSON.stringify(tally))
    }
  }

  console.log(`${label}: waiting for referendum to progress...`)

  const dev = await isDevChain(api)
  if (dev) {
    await (api.rpc as any)('dev_newBlock', { count: 10 })
  } else {
    // On live chains, poll until the referendum is no longer ongoing
    const pollInterval = 6_000 // ~1 block time
    const pollTimeout = 600_000 // 10 minutes max
    const start = Date.now()
    while (Date.now() - start < pollTimeout) {
      const info = await api.query.referenda.referendumInfoFor(referendumIndex)
      const human = info.toHuman() as any
      console.log(
        `${label}: referendum ${referendumIndex} status:`,
        JSON.stringify(human),
      )
      if (human?.Approved || human?.Confirmed || human?.Executed) {
        break
      }
      // If it's still Ongoing, wait for next block
      if (human?.Ongoing) {
        await new Promise((r) => setTimeout(r, pollInterval))
        continue
      }
      // Rejected or other terminal state
      break
    }
  }

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
