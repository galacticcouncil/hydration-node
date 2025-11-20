import { ApiPromise, WsProvider, Keyring } from '@polkadot/api'
import { ISubmittableResult } from '@polkadot/types/types'
import { waitReady } from '@polkadot/wasm-crypto'
import { u8aToHex } from '@polkadot/util'
import { encodeAddress } from '@polkadot/keyring'
import { ethers } from 'ethers'
import { SignetClient } from './signet-client'
import { KeyDerivation } from './key-derivation'
import { blake2AsHex } from '@polkadot/util-crypto'
import { SubmittableExtrinsic } from '@polkadot/api/types'

const isSepolia = true

const WS_ENDPOINT = 'ws://127.0.0.1:8000'
const SEPOLIA_RPC = 'https://ethereum-sepolia-rpc.publicnode.com'
const ANVIL_RPC = 'http://localhost:8545'
const ANVIL_PUBLIC_KEY =
  '0x048318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed753547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5'
const SEPOLIA_PUBLIC_KEY =
  '0x047ca560e19ef0fb49f046670e50b6ceb394122ddfed5526802e5e438cdd2bc5347963e633398aa8498e8711c416746d87d49a8860e04967761d0a0cea229a5220'
const SEPOLIA_CHAIN_ID = 11155111
const ANVIL_CHAIN_ID = 31337
const CHAIN_ID = 'polkadot:2034'
const ANVIL_FAUCET_ADDRESS = '0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512'
const SEPOLIA_FAUCET_ADDRESS = '0x52Be077e67496c9763cCEF66c1117dD234Ca8Cfc'
const LOCAL_SS58_PREFIX = 0

const RPC_URL = isSepolia ? SEPOLIA_RPC : ANVIL_RPC
const ROOT_PUBLIC_KEY = isSepolia ? SEPOLIA_PUBLIC_KEY : ANVIL_PUBLIC_KEY
const EVM_CHAIN_ID = isSepolia ? SEPOLIA_CHAIN_ID : ANVIL_CHAIN_ID
const FAUCET_ADDRESS = isSepolia ? SEPOLIA_FAUCET_ADDRESS : ANVIL_FAUCET_ADDRESS

const MIN_BOB_NATIVE_BALANCE = 1_000_000_000_000n
const PALLET_MIN_NATIVE_BALANCE = 10_000_000_000_000n
const BOB_NATIVE_TOPUP = 100_000_000_000_000n
const PALLET_FAUCET_FUND = ethers.parseEther('100')
const REQUEST_FUND_AMOUNT = ethers.parseEther('0.000001')

const TARGET_ADDRESS = '0x7f67681ce8c292bbbef0ccfa1475d9742b6ab3ac'

const GAS_LIMIT = 100_000n
const DEFAULT_MAX_FEE_PER_GAS = 30_000_000_000n
const DEFAULT_MAX_PRIORITY_FEE_PER_GAS = 2_000_000_000n

const PALLET_ID_STR = 'py/fucet'
const MODL_PREFIX = 'modl'

function getPalletAccountId(): Uint8Array {
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
  maxRetries: number = 1,
  timeoutMs: number = 60_000
): Promise<{ events: any[] }> {
  let attempt = 0

  while (attempt <= maxRetries) {
    try {
      console.log(`${label} - Attempt ${attempt + 1}/${maxRetries + 1}`)

      const result = await new Promise<{ events: any[] }>((resolve, reject) => {
        let unsubscribe: any

        const timer = setTimeout(() => {
          if (unsubscribe) unsubscribe()
          console.log(`⏱️  ${label} timed out after ${timeoutMs}ms`)
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
                `✅ ${label} included in block ${status.asInBlock.toHex()}`
              )

              console.log('------111', label)

              if (dispatchError) {
                if (dispatchError.isModule) {
                  const decoded = api.registry.findMetaError(
                    dispatchError.asModule
                  )
                  reject(
                    new Error(
                      `${decoded.section}.${decoded.name}: ${decoded.docs.join(
                        ' '
                      )}`
                    )
                  )
                } else {
                  reject(new Error(dispatchError.toString()))
                }
                return
              }
              console.log('------112')

              resolve({ events: Array.from(events) })
            } else if (status.isInvalid) {
              clearTimeout(timer)
              if (unsubscribe) unsubscribe()
              console.log(`⚠️  ${label} marked as Invalid`)
              reject(new Error('INVALID_TX'))
            } else if (status.isDropped) {
              clearTimeout(timer)
              if (unsubscribe) unsubscribe()
              reject(new Error(`${label} dropped`))
            }
          }
        )
          .then((unsub: any) => {
            unsubscribe = unsub
          })
          .catch((error: any) => {
            clearTimeout(timer)
            reject(error)
          })
      })

      return result
    } catch (error: any) {
      if (
        (error.message === 'INVALID_TX' || error.message === 'TIMEOUT') &&
        attempt < maxRetries
      ) {
        console.log(`🔄 Retrying ${label}...`)
        attempt++
        await new Promise((resolve) => setTimeout(resolve, 2_000))
        continue
      }
      throw error
    }
  }

  throw new Error(`${label} failed after ${maxRetries + 1} attempts`)
}

function ethAddressFromPubKey(pubKey: string): string {
  const hash = ethers.keccak256('0x' + pubKey.slice(4))
  return '0x' + hash.slice(-40)
}

function constructSignedTransaction(
  unsignedSerialized: string,
  signature: any
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

async function waitForReadResponse(
  api: ApiPromise,
  requestId: string,
  timeout: number
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
          if (event.section === 'signet' && event.method === 'ReadResponded') {
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

async function getTokenFree(
  api: ApiPromise,
  who: string,
  assetId: number
): Promise<bigint> {
  const acc = await api.query.tokens.accounts(who, assetId)
  return (acc as any).free as unknown as bigint
}

async function transferAsset(
  api: ApiPromise,
  from: any,
  to: string,
  assetId: number,
  amount: bigint | string,
  label: string
) {
  const tx = api.tx.tokens.transfer(to, assetId, amount)
  await submitWithRetry(tx, from, api, label)
}

async function createApi(): Promise<ApiPromise> {
  return ApiPromise.create({
    provider: new WsProvider(WS_ENDPOINT),
    types: {
      AffinePoint: { x: '[u8; 32]', y: '[u8; 32]' },
      Signature: { big_r: 'AffinePoint', s: '[u8; 32]', recovery_id: 'u8' },
    },
  })
}

function createKeyringAndAccounts() {
  const keyring = new Keyring({ type: 'sr25519' })
  const alice = keyring.addFromUri('//Alice')
  const bob = keyring.addFromUri('//Bob')
  return { keyring, alice, bob }
}

async function ensureBobHasAssets(
  api: ApiPromise,
  alice: any,
  bob: any,
  faucetAsset: number
) {
  const { data: bobBalance } = (await api.query.system.account(
    bob.address
  )) as any

  if (bobBalance.free.toBigInt() < MIN_BOB_NATIVE_BALANCE) {
    console.log("Funding Bob's account for server responses...")

    await transferAsset(
      api,
      alice,
      bob.address,
      faucetAsset,
      ethers.parseEther('100'),
      `Fund faucet asset ${faucetAsset} to Bob`
    )

    const bobFundTx = api.tx.balances.transferKeepAlive(
      bob.address,
      BOB_NATIVE_TOPUP
    )
    await submitWithRetry(bobFundTx, alice, api, 'Fund Bob account')
  }
}

async function logAliceTokenBalances(
  api: ApiPromise,
  alice: any,
  faucetAsset: number,
  feeAsset: number
) {
  const faucetBal = await getTokenFree(api, alice.address, faucetAsset)
  const feeBal = await getTokenFree(api, alice.address, feeAsset)

  console.log(
    'Alice balances:',
    'faucetBalance =',
    faucetBal.toString(),
    'feeBalance =',
    feeBal.toString()
  )
}

async function fundPalletAccounts(
  api: ApiPromise,
  alice: any,
  faucetAsset: number
): Promise<{ palletSS58: string }> {
  const palletAccountId = getPalletAccountId()
  const palletSS58 = encodeAddress(palletAccountId, LOCAL_SS58_PREFIX)

  await transferAsset(
    api,
    alice,
    palletSS58,
    faucetAsset,
    PALLET_FAUCET_FUND,
    `Fund pallet faucet asset ${faucetAsset}`
  )

  const { data: palletBalance } = (await api.query.system.account(
    palletSS58
  )) as any

  if (palletBalance.free.toBigInt() < PALLET_MIN_NATIVE_BALANCE) {
    console.log(`Funding pallet native balance ${palletSS58}...`)

    const fundTx = api.tx.balances.transferKeepAlive(
      palletSS58,
      PALLET_MIN_NATIVE_BALANCE
    )
    await submitWithRetry(fundTx, alice, api, 'Fund pallet account')
  }

  return { palletSS58 }
}

function deriveSubstrateAndEthAddresses(
  keyring: Keyring,
  alice: any,
  palletSS58: string
): { derivedPubKey: string; derivedEthAddress: string; aliceHexPath: string } {
  const aliceAccountId = keyring.decodeAddress(alice.address)
  const aliceHexPath = '0x' + u8aToHex(aliceAccountId).slice(2)

  const derivedPubKey = KeyDerivation.derivePublicKey(
    ROOT_PUBLIC_KEY,
    palletSS58,
    aliceHexPath,
    CHAIN_ID
  )

  const derivedEthAddress = ethAddressFromPubKey(derivedPubKey)

  console.log(`\n🔑 Derived Ethereum Address: ${derivedEthAddress}`)

  return { derivedPubKey, derivedEthAddress, aliceHexPath }
}

async function ensureDerivedEthHasGas(
  provider: ethers.JsonRpcProvider,
  derivedEthAddress: string
) {
  const ethBalance = await provider.getBalance(derivedEthAddress)
  const feeData = await provider.getFeeData()

  const maxFeePerGas = feeData.maxFeePerGas || DEFAULT_MAX_FEE_PER_GAS
  const estimatedGas = maxFeePerGas * GAS_LIMIT

  console.log(`💰 Balances for ${derivedEthAddress}:`)
  console.log(`   ETH: ${ethers.formatEther(ethBalance)}`)
  console.log(
    `   Estimated gas needed: ${ethers.formatEther(estimatedGas)} ETH\n`
  )

  if (ethBalance < estimatedGas) {
    throw new Error(
      `❌ Insufficient ETH at ${derivedEthAddress}\n` +
        `   Need: ${ethers.formatEther(estimatedGas)} ETH\n` +
        `   Have: ${ethers.formatEther(ethBalance)} ETH\n` +
        `   Please fund this address with ETH for gas`
    )
  }
}

async function initializeVaultIfNeeded(api: ApiPromise, alice: any) {
  const existingConfig = await api.query.ethDispenser.dispenserConfig()
  const configJson = existingConfig.toJSON()

  console.log('Existing dispenser config JSON -> ', configJson)

  if (configJson !== null) {
    console.log('⚠️  Vault already initialized, skipping initialization')
    console.log('   Existing config:', existingConfig.toHuman())
    return
  }

  const mpcEthAddress = ethAddressFromPubKey(ROOT_PUBLIC_KEY)
  console.log('Initializing vault with MPC address (via Root):', mpcEthAddress)

  const initCall = api.tx.ethDispenser.initialize(PALLET_FAUCET_FUND)

  await executeAsRootViaScheduler(
    api,
    initCall,
    'Initialize ethDispenser via Root'
  )

  const cfg = await api.query.ethDispenser.dispenserConfig()
  console.log('Dispenser config after Root init:', cfg.toHuman())
}

describe('ERC20 Vault Integration', () => {
  let api: ApiPromise
  let alice: any
  let signetClient: SignetClient
  let sepoliaProvider: ethers.JsonRpcProvider
  let derivedEthAddress: string
  let derivedPubKey: string
  let aliceHexPath: string
  let palletSS58: string

  beforeAll(async () => {
    await waitReady()

    api = await createApi()

    const feeAsset = (api.consts.ethDispenser.feeAsset as any).toNumber()
    const faucetAsset = (api.consts.ethDispenser.faucetAsset as any).toNumber()

    console.log(
      `feeAsset = ${feeAsset}
      faucetAsset = ${faucetAsset}
      faucetAddress = ${api.consts.ethDispenser.faucetAddress.toString()}`
    )

    const { keyring, alice: aliceAcc, bob } = createKeyringAndAccounts()
    alice = aliceAcc

    await logAliceTokenBalances(api, alice, faucetAsset, feeAsset)
    await ensureBobHasAssets(api, alice, bob, faucetAsset)

    const palletFunding = await fundPalletAccounts(api, alice, faucetAsset)
    palletSS58 = palletFunding.palletSS58

    signetClient = new SignetClient(api, alice)
    sepoliaProvider = new ethers.JsonRpcProvider(RPC_URL)

    await signetClient.ensureSignetInitializedViaReferendum(
      api,
      alice,
      CHAIN_ID
    )

    console.log('----1 ')

    const derived = deriveSubstrateAndEthAddresses(keyring, alice, palletSS58)
    derivedEthAddress = derived.derivedEthAddress
    derivedPubKey = derived.derivedPubKey
    aliceHexPath = derived.aliceHexPath

    console.log('----2 ')
    await ensureDerivedEthHasGas(sepoliaProvider, derivedEthAddress)
    console.log('----3 ')
  }, 120_000)

  afterAll(async () => {
    if (api) {
      await api.disconnect()
    }
  })

  it('should complete full deposit and claim flow', async () => {
    await initializeVaultIfNeeded(api, alice)

    const feeData = await sepoliaProvider.getFeeData()
    const currentNonce = await sepoliaProvider.getTransactionCount(
      derivedEthAddress,
      'pending'
    )

    console.log(`📊 Current nonce for ${derivedEthAddress}: ${currentNonce}`)

    const txParams = {
      value: 0,
      gasLimit: Number(GAS_LIMIT),
      maxFeePerGas: Number(feeData.maxFeePerGas || DEFAULT_MAX_FEE_PER_GAS),
      maxPriorityFeePerGas: Number(
        feeData.maxPriorityFeePerGas || DEFAULT_MAX_PRIORITY_FEE_PER_GAS
      ),
      nonce: currentNonce,
      chainId: EVM_CHAIN_ID,
    }

    console.log({
      derivedEthAddress,
      aliceHexPath,
    })

    const iface = new ethers.Interface([
      'function fund(address to, uint256 amount) external',
    ])

    const data = iface.encodeFunctionData('fund', [
      TARGET_ADDRESS,
      REQUEST_FUND_AMOUNT,
    ])

    const tx = ethers.Transaction.from({
      type: 2,
      chainId: txParams.chainId,
      nonce: txParams.nonce,
      maxPriorityFeePerGas: txParams.maxPriorityFeePerGas,
      maxFeePerGas: txParams.maxFeePerGas,
      gasLimit: txParams.gasLimit,
      to: FAUCET_ADDRESS,
      value: 0,
      data,
    })

    const requestId = signetClient.calculateSignRespondRequestId(
      palletSS58,
      Array.from(ethers.getBytes(tx.unsignedSerialized)),
      {
        slip44ChainId: 60,
        keyVersion: 0,
        path: aliceHexPath,
        algo: 'ecdsa',
        dest: 'ethereum',
        params: '',
      }
    )

    console.log(`📋 Request ID: ${ethers.hexlify(requestId)}\n`)

    const requestIdBytes =
      typeof requestId === 'string' ? ethers.getBytes(requestId) : requestId

    const depositTx = api.tx.ethDispenser.requestFund(
      Array.from(ethers.getBytes(TARGET_ADDRESS)),
      REQUEST_FUND_AMOUNT.toString(),
      requestIdBytes,
      txParams
    )

    console.log('🚀 Submitting deposit_erc20 transaction...')
    const depositResult = await submitWithRetry(
      depositTx,
      alice,
      api,
      'Request Fund'
    )

    const signetEvents = depositResult.events.filter(
      (record: any) =>
        record.event.section === 'signet' &&
        record.event.method === 'SignRespondRequested'
    )

    console.log(`📊 Found ${signetEvents.length} SignRespondRequested event(s)`)

    if (signetEvents.length > 0) {
      console.log(
        '✅ SignRespondRequested event emitted - MPC should pick it up!'
      )
    } else {
      console.log('⚠️  No SignRespondRequested event found!')
    }

    console.log('⏳ Waiting for MPC signature...')

    const signature = await signetClient.waitForSignature(
      ethers.hexlify(requestId),
      1_200_000
    )

    if (!signature) {
      throw new Error('❌ Timeout waiting for MPC signature')
    }

    console.log(`✅ Received signature from: ${signature.responder}\n`)

    const signedTx = constructSignedTransaction(
      tx.unsignedSerialized,
      signature.signature
    )
    const recoveredTx = ethers.Transaction.from(signedTx)
    const recoveredAddress = recoveredTx.from

    console.log(`🔍 Signature verification:`)
    console.log(`   Expected address: ${derivedEthAddress}`)
    console.log(`   Recovered address: ${recoveredAddress}`)
    console.log(
      `   Match: ${
        recoveredAddress?.toLowerCase() === derivedEthAddress.toLowerCase()
      }`
    )

    if (recoveredAddress?.toLowerCase() !== derivedEthAddress.toLowerCase()) {
      throw new Error(
        `❌ Signature verification failed!\n` +
          `   Expected: ${derivedEthAddress}\n` +
          `   Recovered: ${recoveredAddress}\n` +
          `   This means the MPC signed with the wrong key or recovery ID is incorrect.`
      )
    }

    const freshNonce = await sepoliaProvider.getTransactionCount(
      derivedEthAddress,
      'pending'
    )
    console.log(`📊 Fresh nonce check: ${freshNonce}`)

    if (freshNonce !== txParams.nonce) {
      throw new Error(
        `❌ Nonce mismatch! Expected ${txParams.nonce}, but network shows ${freshNonce}.\n` +
          `   A transaction may have already been sent from this address.`
      )
    }

    console.log('📡 Broadcasting transaction to Sepolia...')
    const txResponse = await sepoliaProvider.broadcastTransaction(signedTx)
    console.log(`   Tx Hash: ${txResponse.hash}`)

    const receipt = await txResponse.wait()
    console.log(`✅ Transaction confirmed in block ${receipt?.blockNumber}\n`)

    console.log('⏳ Waiting for MPC to read transaction result...')
    const readResponse = await waitForReadResponse(
      api,
      ethers.hexlify(requestId),
      120_000
    )

    if (!readResponse) {
      throw new Error('❌ Timeout waiting for read response')
    }

    console.log('✅ Received read response\n')

    console.log('\n🔍 Claim Debug:')
    console.log('  Request ID:', ethers.hexlify(requestIdBytes))
    console.log(
      '  Output (hex):',
      Buffer.from(readResponse.output).toString('hex')
    )
  }, 180_000)
})

export async function executeAsRootViaReferendum(
  api: ApiPromise,
  signer: any, // e.g. alice
  call: any, // api.tx.<pallet>.<fn>(...)
  label: string,
  maxRetries = 1,
  timeoutMs = 60_000
): Promise<number> {
  console.log(`\n=== ${label}: starting Root execution via Referenda ===`)

  // 1) Encode call & hash it for preimage + Lookup
  const encodedCall = call.method.toHex()
  const encodedHash = blake2AsHex(encodedCall)

  console.log(`${label}: encodedCall = ${encodedCall}`)
  console.log(`${label}: encodedHash = ${encodedHash}`)

  // 2) Note preimage
  console.log(`${label}: noting preimage...`)
  const notePreimageTx = api.tx.preimage.notePreimage(encodedCall)
  await submitWithRetry(
    notePreimageTx,
    signer,
    api,
    `${label} - notePreimage`,
    maxRetries,
    timeoutMs
  )

  // 3) Submit referendum with ROOT origin
  console.log(`${label}: submitting referendum with Root origin...`)
  const proposalOrigin = { system: 'Root' }
  const proposalCall = {
    Lookup: {
      hash: encodedHash,
      // same length formula as in the Hydration runtime-upgrade tests
      len: encodedCall.length / 2 - 1,
    },
  }
  const enactmentMoment = { After: 1 }

  const submitTx = api.tx.referenda.submit(
    proposalOrigin,
    proposalCall,
    enactmentMoment
  )

  await submitWithRetry(
    submitTx,
    signer,
    api,
    `${label} - submitReferendum`,
    maxRetries,
    timeoutMs
  )

  const referendumIndex =
    parseInt((await api.query.referenda.referendumCount()).toString()) - 1
  console.log(`${label}: referendumIndex = ${referendumIndex}`)

  const faucetAsset = (api.consts.ethDispenser.faucetAsset as any).toNumber()

  let { data } = (await api.query.system.account(signer.address)) as any
  console.log('signer free balance =', data.free.toBigInt().toString())
  const acc = (await api.query.tokens.accounts(
    signer.address,
    faucetAsset
  )) as any
  console.log(
    'faucet asset id =',
    faucetAsset,
    'faucet asset free =',
    acc.free.toString()
  )

  const tracks: any = api.consts.referenda.tracks
  console.log('Tracks:', tracks.toHuman())

  // 5) Place decision deposit
  console.log(`${label}: placing decision deposit...`)
  const decisionDepositTx =
    api.tx.referenda.placeDecisionDeposit(referendumIndex)
  await submitWithRetry(
    decisionDepositTx,
    signer,
    api,
    `${label} - decisionDeposit`,
    maxRetries,
    timeoutMs
  )

  // 6) Vote AYE with a big balance so it passes quickly
  console.log(`${label}: voting AYE on referendum...`)
  data = ((await api.query.system.account(signer.address)) as any).data
  const free = data.free.toBigInt()

  const voteAmount = (free * 5n) / 10n

  console.log(
    `${label}: free balance = ${free.toString()}, voteAmount = ${voteAmount.toString()}`
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
    timeoutMs
  )

  console.log(`${label}: waiting for referendum to progress...`)

  await (api.rpc as any)('dev_newBlock', { count: 10 })
  const info = await api.query.referenda.referendumInfoFor(referendumIndex)
  console.log('Referendum info:', info.toHuman())

  console.log(
    `=== ${label}: Root call scheduled via referenda (index ${referendumIndex}) ===\n`
  )

  // You can return the index to inspect later if needed
  return referendumIndex
}

export async function executeAsRootViaScheduler(
  api: ApiPromise,
  call: SubmittableExtrinsic<'promise'>,
  label: string
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

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms))
}
