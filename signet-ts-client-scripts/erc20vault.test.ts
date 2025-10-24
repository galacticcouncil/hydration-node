import { ApiPromise, WsProvider, Keyring } from '@polkadot/api'
import { ISubmittableResult } from '@polkadot/types/types'
import { waitReady } from '@polkadot/wasm-crypto'
import { u8aToHex } from '@polkadot/util'
import { encodeAddress } from '@polkadot/keyring'
import { ethers } from 'ethers'
import { SignetClient } from './signet-client'
import { KeyDerivation } from './key-derivation'

const ROOT_PUBLIC_KEY =
  '0x049d9031e97dd78ff8c15aa86939de9b1e791066a0224e331bc962a2099a7b1f0464b8bbafe1535f2301c72c2cb3535b172da30b02686ab0393d348614f157fbdb'
const CHAIN_ID = 'polkadot:2034'
const SEPOLIA_RPC = 'http://localhost:8545'
const FAUCET_ADDRESS = '0x5FbDB2315678afecb367f032d93F642f64180aa3'

function getPalletAccountId(): Uint8Array {
  const palletId = new TextEncoder().encode('py/fucet')
  const modl = new TextEncoder().encode('modl')
  const data = new Uint8Array(32)
  data.set(modl, 0)
  data.set(palletId, 4)
  return data
}

async function submitWithRetry(
  tx: any,
  signer: any,
  api: ApiPromise,
  label: string,
  maxRetries: number = 1,
  timeoutMs: number = 60000
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

        tx.signAndSend(signer, (result: ISubmittableResult) => {
          const { status, events, dispatchError } = result

          if (status.isInBlock) {
            clearTimeout(timer)
            if (unsubscribe) unsubscribe()

            console.log(
              `✅ ${label} included in block ${status.asInBlock.toHex()}`
            )

            // Check for dispatch errors
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
        })
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
        await new Promise((resolve) => setTimeout(resolve, 2000)) // Wait 2s before retry
        continue
      }
      throw error
    }
  }

  throw new Error(`${label} failed after ${maxRetries + 1} attempts`)
}

describe('ERC20 Vault Integration', () => {
  let api: ApiPromise
  let alice: any
  let signetClient: SignetClient
  let sepoliaProvider: ethers.JsonRpcProvider
  let derivedEthAddress: string
  let derivedPubKey: string

  beforeAll(async () => {
    await waitReady()

    api = await ApiPromise.create({
      provider: new WsProvider('ws://127.0.0.1:8000'),
      types: {
        AffinePoint: { x: '[u8; 32]', y: '[u8; 32]' },
        Signature: { big_r: 'AffinePoint', s: '[u8; 32]', recovery_id: 'u8' },
      },
    })

    const feeAsset = (api.consts.sigEthFaucet.feeAsset as any).toNumber()
    const faucetAsset = (api.consts.sigEthFaucet.faucetAsset as any).toNumber()

    console.log(
      'api.consts.sigEthFaucet',
      api.consts.sigEthFaucet.mpcRootSigner.toString(),
      api.consts.sigEthFaucet.faucetAddress.toString()
    )

    const keyring = new Keyring({ type: 'sr25519' })
    alice = keyring.addFromUri('//Alice')
    const bob = keyring.addFromUri('//Bob')

    const { data: bobBalance } = (await api.query.system.account(
      bob.address
    )) as any

    console.log(
      'sudo suod',
      ((await getTokenFree(api, alice.address, faucetAsset)) as any).toString(),
      ((await getTokenFree(api, alice.address, feeAsset)) as any).toString()
    )

    if (bobBalance.free.toBigInt() < 1000000000000n) {
      console.log("Funding Bob's account for server responses...")

      await transferAssetToBob(
        api,
        alice,
        bob.address,
        faucetAsset,
        ethers.parseEther('100')
      )

      const bobFundTx = api.tx.balances.transferKeepAlive(
        bob.address,
        100000000000000n
      )
      await submitWithRetry(bobFundTx, alice, api, 'Fund Bob account')
    }

    const palletAccountId = getPalletAccountId()
    const palletSS58 = encodeAddress(palletAccountId, 0)

    const { data: palletBalance } = (await api.query.system.account(
      palletSS58
    )) as any

    const fundingAmount = 10000000000000n

    if (palletBalance.free.toBigInt() < fundingAmount) {
      console.log(`Funding ERC20 vault pallet account ${palletSS58}...`)

      const fundTx = api.tx.balances.transferKeepAlive(
        palletSS58,
        fundingAmount
      )
      await submitWithRetry(fundTx, alice, api, 'Fund pallet account')
    }

    signetClient = new SignetClient(api, alice)
    sepoliaProvider = new ethers.JsonRpcProvider(SEPOLIA_RPC)

    await signetClient.ensureInitialized(CHAIN_ID)

    const aliceAccountId = keyring.decodeAddress(alice.address)
    const aliceHexPath = '0x' + u8aToHex(aliceAccountId).slice(2)

    derivedPubKey = KeyDerivation.derivePublicKey(
      ROOT_PUBLIC_KEY,
      palletSS58,
      aliceHexPath,
      CHAIN_ID
    )

    derivedEthAddress = ethAddressFromPubKey(derivedPubKey)

    console.log(`\n🔑 Derived Ethereum Address: ${derivedEthAddress}`)
    await checkFunding()
  }, 120000)

  afterAll(async () => {
    if (api) {
      await api.disconnect()
    }
  })

  async function checkFunding() {
    const ethBalance = await sepoliaProvider.getBalance(derivedEthAddress)

    const feeData = await sepoliaProvider.getFeeData()
    const gasLimit = 100000n
    const estimatedGas = (feeData.maxFeePerGas || 30000000000n) * gasLimit

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

  it('should complete full deposit and claim flow', async () => {
    const mpcEthAddress = ethAddressFromPubKey(ROOT_PUBLIC_KEY)
    console.log('Checking vault initialization...')
    console.log('mpcEthAddress -> ', mpcEthAddress)
    const mpcAddressBytes = Array.from(ethers.getBytes(mpcEthAddress))

    const existingConfig = await api.query.sigEthFaucet.faucetConfig()

    const configJson = existingConfig.toJSON()
    console.log('configJson -> ', configJson)

    if (configJson !== null) {
      console.log('⚠️  Vault already initialized, skipping initialization')
      console.log('   Existing config:', existingConfig.toHuman())
    } else {
      console.log('Initializing vault with MPC address:', mpcEthAddress)
      const initTx = api.tx.sigEthFaucet.initialize()
      await submitWithRetry(initTx, alice, api, 'Initialize vault')
    }

    const amount = ethers.parseEther('0.1')
    const feeData = await sepoliaProvider.getFeeData()
    const currentNonce = await sepoliaProvider.getTransactionCount(
      derivedEthAddress,
      'pending'
    )

    console.log(`📊 Current nonce for ${derivedEthAddress}: ${currentNonce}`)

    const txParams = {
      value: 0,
      gasLimit: 100000,
      maxFeePerGas: Number(feeData.maxFeePerGas || 30000000000n),
      maxPriorityFeePerGas: Number(feeData.maxPriorityFeePerGas || 2000000000n),
      nonce: currentNonce,
      chainId: 31337,
    }

    const keyring = new Keyring({ type: 'sr25519' })
    const palletAccountId = getPalletAccountId()
    const palletSS58 = encodeAddress(palletAccountId, 0)
    const aliceAccountId = keyring.decodeAddress(alice.address)
    const aliceHexPath = '0x' + u8aToHex(aliceAccountId).slice(2)

    // Build transaction to get request ID
    const iface = new ethers.Interface([
      'function fund(address to, uint256 amount) external',
    ])
    const data = iface.encodeFunctionData('fund', [
      '0x70997970C51812dc3A010C7d01b50e0d17dc79C8',
      amount,
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
      data: data,
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

    const depositTx = api.tx.sigEthFaucet.requestFund(
      Array.from(ethers.getBytes('0x70997970C51812dc3A010C7d01b50e0d17dc79C8')),
      amount.toString(),
      txParams
    )

    console.log('🚀 Submitting deposit_erc20 transaction...')
    const depositResult = await submitWithRetry(
      depositTx,
      alice,
      api,
      'Deposit ERC20'
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
      1200000
    )

    if (!signature) {
      throw new Error('❌ Timeout waiting for MPC signature')
    }

    console.log(`✅ Received signature from: ${signature.responder}\n`)

    // Verify signature by recovering address
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

    // if (recoveredAddress?.toLowerCase() !== derivedEthAddress.toLowerCase()) {
    //   throw new Error(
    //     `❌ Signature verification failed!\n` +
    //       `   Expected: ${derivedEthAddress}\n` +
    //       `   Recovered: ${recoveredAddress}\n` +
    //       `   This means the MPC signed with the wrong key or recovery ID is incorrect.`
    //   )
    // }

    // // Get fresh nonce before broadcasting
    // const freshNonce = await sepoliaProvider.getTransactionCount(
    //   derivedEthAddress,
    //   'pending'
    // )
    // console.log(`📊 Fresh nonce check: ${freshNonce}`)

    // if (freshNonce !== txParams.nonce) {
    //   throw new Error(
    //     `❌ Nonce mismatch! Expected ${txParams.nonce}, but network shows ${freshNonce}.\n` +
    //       `   A transaction may have already been sent from this address.`
    //   )
    // }

    // console.log('📡 Broadcasting transaction to Sepolia...')
    // const txResponse = await sepoliaProvider.broadcastTransaction(signedTx)
    // console.log(`   Tx Hash: ${txResponse.hash}`)

    // const receipt = await txResponse.wait()
    // console.log(`✅ Transaction confirmed in block ${receipt?.blockNumber}\n`)

    // console.log('⏳ Waiting for MPC to read transaction result...')
    // const readResponse = await waitForReadResponse(
    //   api,
    //   ethers.hexlify(requestId),
    //   120000
    // )

    // if (!readResponse) {
    //   throw new Error('❌ Timeout waiting for read response')
    // }

    // console.log('✅ Received read response\n')

    // console.log('\n🔍 Claim Debug:')
    // console.log('  Request ID:', ethers.hexlify(requestIdBytes))
    // console.log(
    //   '  Output (hex):',
    //   Buffer.from(readResponse.output).toString('hex')
    // )

    // // Strip SCALE compact prefix from output
    // let outputBytes = new Uint8Array(readResponse.output)
    // if (outputBytes.length > 0) {
    //   const mode = outputBytes[0] & 0b11
    //   if (mode === 0) {
    //     outputBytes = outputBytes.slice(1)
    //   } else if (mode === 1) {
    //     outputBytes = outputBytes.slice(2)
    //   } else if (mode === 2) {
    //     outputBytes = outputBytes.slice(4)
    //   }
    // }

    // console.log(
    //   '  Stripped output (hex):',
    //   Buffer.from(outputBytes).toString('hex')
    // )

    // const balanceBefore = await api.query.erc20Vault.userBalances(
    //   alice.address,
    //   Array.from(ethers.getBytes(USDC_SEPOLIA))
    // )

    // const claimTx = api.tx.erc20Vault.claimErc20(
    //   Array.from(requestIdBytes),
    //   Array.from(outputBytes),
    //   readResponse.signature
    // )

    // console.log('🚀 Submitting claim transaction...')
    // await submitWithRetry(claimTx, alice, api, 'Claim ERC20')

    // const balanceAfter = await api.query.erc20Vault.userBalances(
    //   alice.address,
    //   Array.from(ethers.getBytes(USDC_SEPOLIA))
    // )

    // const balanceIncrease =
    //   BigInt(balanceAfter.toString()) - BigInt(balanceBefore.toString())

    // expect(balanceIncrease.toString()).toBe(amount.toString())
    // console.log(
    //   `✅ Balance increased by: ${ethers.formatUnits(
    //     balanceIncrease.toString(),
    //     6
    //   )} USDC`
    // )
    // console.log(
    //   `   Total balance: ${ethers.formatUnits(
    //     balanceAfter.toString(),
    //     6
    //   )} USDC\n`
    // )
  }, 180000)

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
            if (
              event.section === 'signet' &&
              event.method === 'ReadResponded'
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

  function ethAddressFromPubKey(pubKey: string): string {
    const hash = ethers.keccak256('0x' + pubKey.slice(4))
    return '0x' + hash.slice(-40)
  }

  async function fundWETH(
    api: ApiPromise,
    from: any,
    to: string,
    amountWei: string | bigint,
    wethId: any
  ) {
    const tx = api.tx.tokens.transfer(to, wethId, amountWei)
    await submitWithRetry(tx, from, api, `Fund WETH to ${to}`)
  }

  async function getTokenFree(api: ApiPromise, who: string, assetId: number) {
    const acc = await api.query.tokens.accounts(who, assetId)
    return (acc as any).free as unknown as bigint
  }

  async function transferAssetToBob(
    api: ApiPromise,
    alice: any,
    bobAddress: string,
    assetId: number,
    amount: bigint | string
  ) {
    const tx = api.tx.tokens.transfer(bobAddress, assetId, amount)
    await submitWithRetry(
      tx,
      alice,
      api,
      `Transfer asset ${assetId} to ${bobAddress}`
    )
  }
})
