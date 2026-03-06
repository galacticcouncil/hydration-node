import { ApiPromise } from '@polkadot/api'
import { waitReady } from '@polkadot/wasm-crypto'
import { ethers } from 'ethers'
import { SignetClient } from './signet-client'
import { ENV } from './env'
import {
  DISPENSER_SIGNING_PATH,
  submitWithRetry,
  constructSignedTransaction,
  waitForReadResponse,
  createApi,
  createKeyringAndAccounts,
  ensureBobHasAssets,
  ensureServerSignerFunded,
  ensureFaucetMpcAddress,
  logAliceTokenBalances,
  fundPalletAccounts,
  deriveEthAddress,
  ensureDerivedEthHasGas,
  initializeVaultIfNeeded,
} from './utils'

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------

describe('ERC20 Vault Integration', () => {
  let api: ApiPromise
  let alice: any
  let signetClient: SignetClient
  let evmProvider: ethers.JsonRpcProvider
  let derivedEthAddress: string
  let derivedPubKey: string
  let palletSS58: string
  let palletSS58Prefix0: string

  beforeAll(async () => {
    await waitReady()

    api = await createApi()

    const feeAsset = (api.consts.ethDispenser.feeAsset as any).toNumber()
    const faucetAsset = (api.consts.ethDispenser.faucetAsset as any).toNumber()

    console.log(
      `feeAsset = ${feeAsset}`,
      `faucetAsset = ${faucetAsset}`,
    )

    const { keyring, alice: aliceAcc, bob } = createKeyringAndAccounts()
    alice = aliceAcc

    await logAliceTokenBalances(api, alice, faucetAsset, feeAsset)
    await ensureBobHasAssets(api, bob, faucetAsset)
    await ensureServerSignerFunded(api, alice, bob)

    const palletFunding = await fundPalletAccounts(api, alice, faucetAsset)
    palletSS58 = palletFunding.palletSS58
    palletSS58Prefix0 = palletFunding.palletSS58Prefix0

    signetClient = new SignetClient(api, alice)
    evmProvider = new ethers.JsonRpcProvider(ENV.EVM_RPC_URL)

    await signetClient.ensureSignetConfigured(
      api,
      alice,
      ENV.SUBSTRATE_CHAIN_ID,
    )

    const derived = deriveEthAddress()
    derivedEthAddress = derived.derivedEthAddress
    derivedPubKey = derived.derivedPubKey

    await ensureDerivedEthHasGas(evmProvider, derivedEthAddress)
    await ensureFaucetMpcAddress(evmProvider, derivedEthAddress)
  }, 600_000)

  afterAll(async () => {
    if (api) {
      await api.disconnect()
    }
  })

  it('should complete full deposit and claim flow', async () => {
    await initializeVaultIfNeeded(api, alice)

    const feeData = await evmProvider.getFeeData()
    const currentNonce = await evmProvider.getTransactionCount(
      derivedEthAddress,
      'pending',
    )

    console.log(`Current nonce for ${derivedEthAddress}: ${currentNonce}`)

    // Add a unique component to maxPriorityFeePerGas so each run produces
    // a different unsigned tx → different request ID (avoids DuplicateRequest)
    const basePriorityFee = Number(
      feeData.maxPriorityFeePerGas || ENV.DEFAULT_MAX_PRIORITY_FEE_PER_GAS,
    )
    const uniquePriorityFee = basePriorityFee + Math.floor(Math.random() * 1_000_000)

    const txParams = {
      value: 0,
      gasLimit: Number(ENV.GAS_LIMIT),
      maxFeePerGas: Number(feeData.maxFeePerGas || ENV.DEFAULT_MAX_FEE_PER_GAS),
      maxPriorityFeePerGas: uniquePriorityFee,
      nonce: currentNonce,
      chainId: ENV.EVM_CHAIN_ID,
    }

    const iface = new ethers.Interface([
      'function fund(address to, uint256 amount) external',
    ])

    const data = iface.encodeFunctionData('fund', [
      ENV.TARGET_ADDRESS,
      ENV.REQUEST_FUND_AMOUNT,
    ])

    // Read faucet address from on-chain config (no longer a compile-time constant)
    const dispenserCfg = await (api.query as any).ethDispenser.dispenserConfig()
    const cfg = dispenserCfg.toJSON() as any
    const faucetAddress = cfg?.faucetAddress || ENV.FAUCET_ADDRESS

    const tx = ethers.Transaction.from({
      type: 2,
      chainId: txParams.chainId,
      nonce: txParams.nonce,
      maxPriorityFeePerGas: txParams.maxPriorityFeePerGas,
      maxFeePerGas: txParams.maxFeePerGas,
      gasLimit: txParams.gasLimit,
      to: faucetAddress,
      value: 0,
      data,
    })

    // Fixed signing path — all users derive the same MPC key
    const requestId = signetClient.calculateSignRespondRequestId(
      palletSS58Prefix0,
      Array.from(ethers.getBytes(tx.unsignedSerialized)),
      {
        caip2_id: `eip155:${ENV.EVM_CHAIN_ID}`,
        keyVersion: 0,
        path: DISPENSER_SIGNING_PATH,
        algo: 'ecdsa',
        dest: 'ethereum',
        params: '',
      },
    )

    console.log(`Request ID: ${ethers.hexlify(requestId)}\n`)

    const requestIdBytes =
      typeof requestId === 'string' ? ethers.getBytes(requestId) : requestId

    const depositTx = api.tx.ethDispenser.requestFund(
      Array.from(ethers.getBytes(ENV.TARGET_ADDRESS)),
      ENV.REQUEST_FUND_AMOUNT.toString(),
      requestIdBytes,
      txParams,
    )

    console.log('Submitting requestFund transaction...')
    const depositResult = await submitWithRetry(
      depositTx,
      alice,
      api,
      'Request Fund',
    )

    const signetEvents = depositResult.events.filter(
      (record: any) =>
        record.event.section === 'signet' &&
        record.event.method === 'SignBidirectionalRequested',
    )

    console.log(
      `Found ${signetEvents.length} SignBidirectionalRequested event(s)`,
    )

    if (signetEvents.length > 0) {
      console.log(
        'SignBidirectionalRequested event emitted - MPC should pick it up',
      )
    } else {
      console.log('No SignBidirectionalRequested event found!')
    }

    console.log('Waiting for MPC signature...')

    const signature = await signetClient.waitForSignature(
      ethers.hexlify(requestId),
      1_200_000,
    )

    if (!signature) {
      throw new Error('Timeout waiting for MPC signature')
    }

    console.log(`Received signature from: ${signature.responder}\n`)

    const signedTx = constructSignedTransaction(
      tx.unsignedSerialized,
      signature.signature,
    )
    const recoveredTx = ethers.Transaction.from(signedTx)
    const recoveredAddress = recoveredTx.from

    console.log(`Signature verification:`)
    console.log(`   Expected address: ${derivedEthAddress}`)
    console.log(`   Recovered address: ${recoveredAddress}`)
    console.log(
      `   Match: ${
        recoveredAddress?.toLowerCase() === derivedEthAddress.toLowerCase()
      }`,
    )

    if (recoveredAddress?.toLowerCase() !== derivedEthAddress.toLowerCase()) {
      throw new Error(
        `Signature verification failed!\n` +
          `   Expected: ${derivedEthAddress}\n` +
          `   Recovered: ${recoveredAddress}\n` +
          `   On chopsticks with mock-signature-host the mock MPC returns a dummy signature.\n` +
          `   Run against a real network (lark/mainnet) with an actual MPC for the full flow.`,
      )
    }

    const freshNonce = await evmProvider.getTransactionCount(
      derivedEthAddress,
      'pending',
    )
    console.log(`Fresh nonce check: ${freshNonce}`)

    if (freshNonce !== txParams.nonce) {
      throw new Error(
        `Nonce mismatch! Expected ${txParams.nonce}, but network shows ${freshNonce}.\n` +
          `   A transaction may have already been sent from this address.`,
      )
    }

    console.log('Broadcasting transaction...')
    const txResponse = await evmProvider.broadcastTransaction(signedTx)
    console.log(`   Tx Hash: ${txResponse.hash}`)

    const receipt = await txResponse.wait()
    console.log(`Transaction confirmed in block ${receipt?.blockNumber}\n`)

    console.log('Waiting for MPC to read transaction result...')
    const readResponse = await waitForReadResponse(
      api,
      ethers.hexlify(requestId),
      180_000,
    )

    if (!readResponse) {
      throw new Error('Timeout waiting for read response')
    }

    console.log('Received read response\n')

    console.log('Claim Debug:')
    console.log('  Request ID:', ethers.hexlify(requestIdBytes))
    console.log(
      '  Output (hex):',
      Buffer.from(readResponse.output).toString('hex'),
    )
  }, 1_200_000)
})
