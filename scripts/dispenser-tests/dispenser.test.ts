import { ApiPromise, Keyring } from '@polkadot/api'
import { waitReady } from '@polkadot/wasm-crypto'
import { ethers } from 'ethers'
import { SignetClient } from './signet-client'
import { ENV } from './env'
import {
  submitWithRetry,
  constructSignedTransaction,
  waitForReadResponse,
  createApi,
  createKeyringAndAccounts,
  ensureAccountHasAssets,
  logTokenBalances,
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
  let requester: any
  let signetClient: SignetClient
  let evmProvider: ethers.JsonRpcProvider
  let derivedEthAddress: string
  let palletSS58: string

  beforeAll(async () => {
    await waitReady()

    api = await createApi()

    const feeAsset = (api.consts.ethDispenser.feeAsset as any).toNumber()
    const faucetAsset = (api.consts.ethDispenser.faucetAsset as any).toNumber()

    const dispenserCfg = await (api.query as any).ethDispenser.dispenserConfig()
    console.log(
      `feeAsset = ${feeAsset}`,
      `faucetAsset = ${faucetAsset}`,
      `dispenserConfig = ${JSON.stringify(dispenserCfg.toJSON())}`,
    )

    const { requester: acc } = createKeyringAndAccounts(ENV.TEST_ACCOUNT_URI)
    requester = acc

    const alice = new Keyring({ type: 'sr25519' }).addFromUri('//Alice')
    const palletFunding = await fundPalletAccounts(api, alice, faucetAsset)

    await ensureAccountHasAssets(api, requester, faucetAsset, feeAsset, ENV.TEST_ACCOUNT_URI)
    await logTokenBalances(api, requester, faucetAsset, feeAsset)
    palletSS58 = palletFunding.palletSS58

    signetClient = new SignetClient(api, requester)
    evmProvider = new ethers.JsonRpcProvider(ENV.EVM_RPC_URL)

    await signetClient.ensureSignetInitializedViaReferendum(
      api,
      requester,
      ENV.SUBSTRATE_CHAIN_ID,
    )

    const derived = deriveEthAddress()
    derivedEthAddress = derived.derivedEthAddress

    await ensureDerivedEthHasGas(evmProvider, derivedEthAddress)
  }, 600_000)

  afterAll(async () => {
    if (api) {
      await api.disconnect()
    }
  })

  it('should complete full deposit and claim flow', async () => {
    await initializeVaultIfNeeded(api)

    const feeData = await evmProvider.getFeeData()
    const currentNonce = await evmProvider.getTransactionCount(
      derivedEthAddress,
      'pending',
    )

    console.log(`Current nonce for ${derivedEthAddress}: ${currentNonce}`)

    const txParams = {
      value: 0,
      gasLimit: Number(ENV.GAS_LIMIT),
      maxFeePerGas: Number(feeData.maxFeePerGas || ENV.DEFAULT_MAX_FEE_PER_GAS),
      maxPriorityFeePerGas: Number(
        feeData.maxPriorityFeePerGas || ENV.DEFAULT_MAX_PRIORITY_FEE_PER_GAS,
      ),
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

    const tx = ethers.Transaction.from({
      type: 2,
      chainId: txParams.chainId,
      nonce: txParams.nonce,
      maxPriorityFeePerGas: txParams.maxPriorityFeePerGas,
      maxFeePerGas: txParams.maxFeePerGas,
      gasLimit: txParams.gasLimit,
      to: ENV.FAUCET_ADDRESS,
      value: 0,
      data,
    })

    const requestId = signetClient.calculateSignRespondRequestId(
      palletSS58,
      Array.from(ethers.getBytes(tx.unsignedSerialized)),
      {
        caip2_id: `eip155:${ENV.EVM_CHAIN_ID}`,
        keyVersion: 0,
        path: 'dispenser',
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
      requester,
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
          `   This means the MPC signed with the wrong key or recovery ID is incorrect.`,
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
      60_000,
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
  }, 180_000)

  // This test uses anvil_setBalance to drain the faucet, so it can only run on Anvil.
  const isAnvil = async (provider: ethers.JsonRpcProvider) => {
    try {
      await provider.send('anvil_nodeInfo', [])
      return true
    } catch {
      return false
    }
  }

  it('should issue IOU voucher tokens when faucet has insufficient ETH', async () => {
    if (!(await isAnvil(evmProvider))) {
      console.log('Skipping voucher test: EVM provider is not Anvil')
      return
    }

    await initializeVaultIfNeeded(api)

    // --- Read GasVoucher address from GasFaucet contract ---
    const faucetContract = new ethers.Contract(
      ENV.FAUCET_ADDRESS,
      [
        'function voucher() view returns (address)',
        'function minEthThreshold() view returns (uint256)',
      ],
      evmProvider,
    )
    const voucherAddress = await faucetContract.voucher()
    console.log(`GasVoucher address: ${voucherAddress}`)

    const voucherContract = new ethers.Contract(
      voucherAddress,
      ['function balanceOf(address) view returns (uint256)'],
      evmProvider,
    )

    // --- Force GasFaucet to have 0 ETH so fund() issues vouchers ---
    const originalFaucetBalance = await evmProvider.getBalance(
      ENV.FAUCET_ADDRESS,
    )
    console.log(
      `GasFaucet original ETH balance: ${ethers.formatEther(originalFaucetBalance)}`,
    )

    await evmProvider.send('anvil_setBalance', [ENV.FAUCET_ADDRESS, '0x0'])
    const drainedBalance = await evmProvider.getBalance(ENV.FAUCET_ADDRESS)
    console.log(
      `GasFaucet ETH balance after drain: ${ethers.formatEther(drainedBalance)}`,
    )

    // --- Read voucher balance before the request ---
    const voucherBalanceBefore = await voucherContract.balanceOf(
      ENV.TARGET_ADDRESS,
    )
    console.log(`Voucher balance before: ${voucherBalanceBefore}`)

    // --- Build the EVM transaction ---
    const feeData = await evmProvider.getFeeData()
    const currentNonce = await evmProvider.getTransactionCount(
      derivedEthAddress,
      'pending',
    )

    console.log(`Current nonce for ${derivedEthAddress}: ${currentNonce}`)

    const txParams = {
      value: 0,
      gasLimit: Number(ENV.GAS_LIMIT),
      maxFeePerGas: Number(feeData.maxFeePerGas || ENV.DEFAULT_MAX_FEE_PER_GAS),
      maxPriorityFeePerGas: Number(
        feeData.maxPriorityFeePerGas || ENV.DEFAULT_MAX_PRIORITY_FEE_PER_GAS,
      ),
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

    const tx = ethers.Transaction.from({
      type: 2,
      chainId: txParams.chainId,
      nonce: txParams.nonce,
      maxPriorityFeePerGas: txParams.maxPriorityFeePerGas,
      maxFeePerGas: txParams.maxFeePerGas,
      gasLimit: txParams.gasLimit,
      to: ENV.FAUCET_ADDRESS,
      value: 0,
      data,
    })

    const requestId = signetClient.calculateSignRespondRequestId(
      palletSS58,
      Array.from(ethers.getBytes(tx.unsignedSerialized)),
      {
        caip2_id: `eip155:${ENV.EVM_CHAIN_ID}`,
        keyVersion: 0,
        path: 'dispenser',
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

    // --- Submit requestFund on Substrate ---
    console.log('Submitting requestFund transaction (voucher path)...')
    await submitWithRetry(depositTx, requester, api, 'Request Fund (voucher)')

    // --- Wait for MPC signature ---
    console.log('Waiting for MPC signature...')
    const signature = await signetClient.waitForSignature(
      ethers.hexlify(requestId),
      1_200_000,
    )

    if (!signature) {
      throw new Error('Timeout waiting for MPC signature')
    }

    console.log(`Received signature from: ${signature.responder}`)

    // --- Broadcast the signed EVM transaction ---
    const signedTx = constructSignedTransaction(
      tx.unsignedSerialized,
      signature.signature,
    )

    console.log('Broadcasting transaction...')
    const txResponse = await evmProvider.broadcastTransaction(signedTx)
    console.log(`   Tx Hash: ${txResponse.hash}`)

    const receipt = await txResponse.wait()
    console.log(`Transaction confirmed in block ${receipt?.blockNumber}\n`)

    // --- Verify VoucherIssued event was emitted (not Funded) ---
    const voucherIssuedTopic = ethers.id('VoucherIssued(address,uint256)')
    const fundedTopic = ethers.id('Funded(address,uint256)')

    const voucherEvents =
      receipt?.logs.filter(
        (log) => log.topics[0] === voucherIssuedTopic,
      ) || []

    const fundedEvents =
      receipt?.logs.filter((log) => log.topics[0] === fundedTopic) || []

    console.log(`VoucherIssued events: ${voucherEvents.length}`)
    console.log(`Funded events: ${fundedEvents.length}`)

    expect(voucherEvents.length).toBeGreaterThan(0)
    expect(fundedEvents.length).toBe(0)

    // --- Verify voucher token balance increased by the requested amount ---
    const voucherBalanceAfter = await voucherContract.balanceOf(
      ENV.TARGET_ADDRESS,
    )
    console.log(`Voucher balance after: ${voucherBalanceAfter}`)

    const increase = voucherBalanceAfter - voucherBalanceBefore
    console.log(`Voucher balance increase: ${increase}`)

    expect(increase).toBe(ENV.REQUEST_FUND_AMOUNT)

    // --- Restore GasFaucet ETH balance ---
    if (originalFaucetBalance > 0n) {
      await evmProvider.send('anvil_setBalance', [
        ENV.FAUCET_ADDRESS,
        ethers.toQuantity(originalFaucetBalance),
      ])
      console.log('Restored GasFaucet ETH balance')
    }
  }, 180_000)
})
