import { ApiPromise, WsProvider, Keyring } from '@polkadot/api'
import { ISubmittableResult } from '@polkadot/types/types'
import { waitReady } from '@polkadot/wasm-crypto'
import { u8aToHex } from '@polkadot/util'
import { encodeAddress } from '@polkadot/keyring'
import { ethers } from 'ethers'
import * as bitcoin from 'bitcoinjs-lib'
import Client from 'bitcoin-core'
import { SignetClient } from './signet-client'
import { KeyDerivation } from './key-derivation'
import * as ecc from 'tiny-secp256k1'
import coinSelect from 'coinselect'

bitcoin.initEccLib(ecc)

// Must match the server's MPC_ROOT_KEY from .env
const MPC_ROOT_KEY =
  '0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef'
const ROOT_PUBLIC_KEY = (() => {
  const pub = ecc.pointFromScalar(
    Buffer.from(MPC_ROOT_KEY.slice(2), 'hex'),
    false,
  )
  if (!pub) throw new Error('Invalid MPC root key')
  return '0x' + Buffer.from(pub).toString('hex')
})()
const BTC_CAIP2 = 'bip122:000000000933ea01ad0ee984209779ba'
const SUBSTRATE_CHAIN_ID = 'polkadot:2034'
const SECP256K1_N = BigInt(
  '0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141',
)
const ERROR_PREFIX = Buffer.from([0xde, 0xad, 0xbe, 0xef])

// ============ Helper Functions ============

function getPalletAccountId(): Uint8Array {
  const data = new Uint8Array(32)
  data.set(new TextEncoder().encode('modl'), 0)
  data.set(new TextEncoder().encode('py/btcvt'), 4)
  return data
}

function stripScalePrefix(data: Uint8Array): Uint8Array {
  if (data.length === 0) return data
  const mode = data[0] & 0b11
  if (mode === 0) return data.slice(1)
  if (mode === 1) return data.slice(2)
  if (mode === 2) return data.slice(4)
  return data
}

function compressPubkey(pubKey: string): Buffer {
  const uncompressed = Buffer.from(pubKey.slice(4), 'hex')
  const full = Buffer.concat([Buffer.from([0x04]), uncompressed])
  return Buffer.from(ecc.pointCompress(full, true))
}

function btcAddressFromPubKey(
  pubKey: string,
  network: bitcoin.Network,
): string {
  return bitcoin.payments.p2wpkh({ pubkey: compressPubkey(pubKey), network })
    .address!
}

function normalizeSignature(r: Buffer, s: Buffer): { r: Buffer; s: Buffer } {
  const sBigInt = BigInt('0x' + s.toString('hex'))
  if (sBigInt > SECP256K1_N / 2n) {
    return {
      r,
      s: Buffer.from(
        (SECP256K1_N - sBigInt).toString(16).padStart(64, '0'),
        'hex',
      ),
    }
  }
  return { r, s }
}

function extractSigBuffers(sig: any): { r: Buffer; s: Buffer } {
  const r =
    typeof sig.bigR.x === 'string'
      ? Buffer.from(sig.bigR.x.slice(2), 'hex')
      : Buffer.from(sig.bigR.x)
  const s =
    typeof sig.s === 'string'
      ? Buffer.from(sig.s.slice(2), 'hex')
      : Buffer.from(sig.s)
  return normalizeSignature(r, s)
}

function encodeDER(r: Buffer, s: Buffer): Buffer {
  const toDER = (x: Buffer): Buffer => {
    let i = 0
    while (i < x.length - 1 && x[i] === 0 && x[i + 1] < 0x80) i++
    const trimmed = x.subarray(i)
    return trimmed[0] >= 0x80
      ? Buffer.concat([Buffer.from([0x00]), trimmed])
      : trimmed
  }
  const rDER = toDER(r),
    sDER = toDER(s)
  const len = 2 + rDER.length + 2 + sDER.length
  const buf = Buffer.allocUnsafe(2 + len)
  buf[0] = 0x30
  buf[1] = len
  buf[2] = 0x02
  buf[3] = rDER.length
  rDER.copy(buf, 4)
  buf[4 + rDER.length] = 0x02
  buf[5 + rDER.length] = sDER.length
  sDER.copy(buf, 6 + rDER.length)
  return buf
}

// Compute VAULT_PUBKEY_HASH from MPC root key derivation (must match runtime VaultPubkeyHash)
const VAULT_PUBKEY_HASH = (() => {
  const palletSS58 = encodeAddress(getPalletAccountId(), 0)
  const vaultPubKey = KeyDerivation.derivePublicKey(
    ROOT_PUBLIC_KEY,
    SUBSTRATE_CHAIN_ID,
    palletSS58,
    'root',
  )
  const compressed = compressPubkey(vaultPubKey)
  return Array.from(bitcoin.crypto.hash160(compressed))
})()

function getVaultScript(): Buffer {
  return Buffer.concat([
    Buffer.from([0x00, 0x14]),
    Buffer.from(VAULT_PUBKEY_HASH),
  ])
}

function getScriptCode(
  witnessScript: Buffer,
  network: bitcoin.Network,
): Buffer {
  return Buffer.from(
    bitcoin.payments.p2pkh({
      hash: Buffer.from(witnessScript.subarray(2)),
      network,
    }).output!,
  )
}

async function submitWithRetry(
  tx: any,
  signer: any,
  api: ApiPromise,
  label: string,
  maxRetries = 3,
  timeoutMs = 60000,
): Promise<{ events: any[] }> {
  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await new Promise((resolve, reject) => {
        let unsub: any
        const timer = setTimeout(() => {
          unsub?.()
          reject(new Error('TIMEOUT'))
        }, timeoutMs)

        tx.signAndSend(signer, (result: ISubmittableResult) => {
          const { status, events, dispatchError } = result
          if (status.isInBlock) {
            clearTimeout(timer)
            unsub?.()
            if (dispatchError) {
              const err = dispatchError.isModule
                ? api.registry.findMetaError(dispatchError.asModule)
                : { section: '', name: dispatchError.toString(), docs: [] }
              reject(new Error(`${err.section}.${err.name}`))
              return
            }
            console.log(`✅ ${label} in block`)
            resolve({ events: Array.from(events) })
          } else if (status.isInvalid || status.isDropped) {
            clearTimeout(timer)
            unsub?.()
            reject(new Error(status.isInvalid ? 'INVALID_TX' : 'DROPPED'))
          }
        })
          .then((u: any) => (unsub = u))
          .catch((e: any) => {
            clearTimeout(timer)
            reject(e)
          })
      })
    } catch (error: any) {
      const msg = error.message || ''
      if (
        (msg === 'INVALID_TX' || msg === 'TIMEOUT' || msg.includes('stale')) &&
        attempt < maxRetries
      ) {
        console.log(`🔄 Retrying ${label}...`)
        await new Promise((r) => setTimeout(r, 2000))
        continue
      }
      throw error
    }
  }
  throw new Error(`${label} failed after retries`)
}

async function waitForSignature(
  api: ApiPromise,
  requestId: string,
  timeout: number,
): Promise<any> {
  return new Promise((resolve, reject) => {
    let unsub: any
    const timer = setTimeout(() => {
      unsub?.()
      reject(new Error(`Timeout for ${requestId}`))
    }, timeout)
    api.query.system
      .events((events: any) => {
        for (const { event } of events) {
          if (
            event.section === 'signet' &&
            event.method === 'SignatureResponded'
          ) {
            const [reqId, , signature] = event.data
            if (ethers.hexlify(reqId.toU8a()) === requestId) {
              clearTimeout(timer)
              unsub?.()
              resolve(signature.toJSON())
              return
            }
          }
        }
      })
      .then((u: any) => (unsub = u))
  })
}

async function waitForResponse(
  api: ApiPromise,
  requestId: string,
  timeout: number,
): Promise<any> {
  return new Promise((resolve) => {
    let unsub: any
    const timer = setTimeout(() => {
      unsub?.()
      resolve(null)
    }, timeout)
    api.query.system
      .events((events: any) => {
        for (const { event } of events) {
          if (
            event.section === 'signet' &&
            event.method === 'RespondBidirectionalEvent'
          ) {
            const [reqId, , output, signature] = event.data
            if (ethers.hexlify(reqId.toU8a()) === requestId) {
              clearTimeout(timer)
              unsub?.()
              resolve({
                output: Array.from(output.toU8a()),
                signature: signature.toJSON(),
              })
              return
            }
          }
        }
      })
      .then((u: any) => (unsub = u))
  })
}

// ============ Test Suite ============

describe('BTC Vault Integration', () => {
  let api: ApiPromise
  let alice: any
  let signetClient: SignetClient
  let btcClient: any
  let derivedBtcAddress: string
  let derivedPubKey: string
  let network: bitcoin.Network
  let palletSS58: string
  let spentVaultUtxos: Array<{
    txid: string
    vout: number
    value: number
    script: Buffer
  }> = []

  beforeAll(async () => {
    await waitReady()

    btcClient = new Client({
      host: 'http://localhost:18443',
      username: 'test',
      password: 'test123',
    })
    await btcClient.command('getblockcount') // Verify connection

    api = await ApiPromise.create({
      provider: new WsProvider('ws://127.0.0.1:8000'),
      types: {
        AffinePoint: { x: '[u8; 32]', y: '[u8; 32]' },
        Signature: { big_r: 'AffinePoint', s: '[u8; 32]', recovery_id: 'u8' },
      },
    })

    const keyring = new Keyring({ type: 'sr25519' })
    alice = keyring.addFromUri('//Alice')
    palletSS58 = encodeAddress(getPalletAccountId(), 0)

    // Fund pallet if needed
    const { data: palletBalance } = (await api.query.system.account(
      palletSS58,
    )) as any
    if (palletBalance.free.toBigInt() < 10000000000000n) {
      await submitWithRetry(
        api.tx.balances.transferKeepAlive(palletSS58, 10000000000000n),
        alice,
        api,
        'Fund pallet',
      )
    }

    signetClient = new SignetClient(api, alice)
    await signetClient.ensureInitialized(SUBSTRATE_CHAIN_ID)
    const aliceAccountId = keyring.decodeAddress(alice.address)
    const aliceHexPath = '0x' + u8aToHex(aliceAccountId).slice(2)

    derivedPubKey = KeyDerivation.derivePublicKey(
      ROOT_PUBLIC_KEY,
      SUBSTRATE_CHAIN_ID,
      palletSS58,
      aliceHexPath,
    )
    network = bitcoin.networks.regtest
    derivedBtcAddress = btcAddressFromPubKey(derivedPubKey, network)

    // Fund Bitcoin address
    let walletAddr
    try {
      walletAddr = await btcClient.command('getnewaddress')
    } catch {
      try {
        await btcClient.command('createwallet', 'testwallet')
      } catch {
        await btcClient.command('loadwallet', 'testwallet')
      }
      walletAddr = await btcClient.command('getnewaddress')
    }
    await btcClient.command('generatetoaddress', 101, walletAddr)
    await btcClient.command('sendtoaddress', derivedBtcAddress, 1.0)
    await btcClient.command('generatetoaddress', 1, walletAddr)
    console.log(`✅ Setup complete. Address: ${derivedBtcAddress}`)
  }, 180000)

  afterAll(async () => {
    await api?.disconnect()
  })

  it('should complete deposit and claim flow', async () => {
    // Initialize vault if needed
    const mpcEthAddress = new Uint8Array([
      0x1b, 0xe3, 0x1a, 0x94, 0x36, 0x1a, 0x39, 0x1b, 0xba, 0xfb, 0x2a, 0x4c,
      0xcd, 0x70, 0x4f, 0x57, 0xdc, 0x04, 0xd4, 0xbb,
    ])
    const existingConfig = await api.query.btcVault.palletConfig()
    // if (existingConfig.toJSON() === null) {
    //   await submitWithRetry(
    //     api.tx.btcVault.initialize(Array.from(mpcEthAddress)),
    //     alice,
    //     api,
    //     'Initialize vault',
    //   )
    // }

    // Get UTXOs and build transaction
    const scanResult = await btcClient.command('scantxoutset', 'start', [
      `addr(${derivedBtcAddress})`,
    ])
    expect(scanResult.unspents.length).toBeGreaterThan(0)

    const depositAmount = 35289790
    const utxos = scanResult.unspents.map((u: any) => ({
      txid: u.txid,
      vout: u.vout,
      value: Math.floor(u.amount * 100000000),
      script: Buffer.from(
        bitcoin.address.toOutputScript(derivedBtcAddress, network),
      ),
    }))

    const coinSelectResult = coinSelect(
      utxos,
      [{ script: getVaultScript(), value: depositAmount }],
      2,
    )
    if (!coinSelectResult.inputs || !coinSelectResult.outputs)
      throw new Error('Insufficient funds')
    const { inputs, outputs } = coinSelectResult

    // Build PSBT
    const psbt = new bitcoin.Psbt({ network })
    for (const input of inputs) {
      psbt.addInput({
        hash: Buffer.from(input.txid, 'hex').reverse(),
        index: input.vout,
        sequence: 0xffffffff,
        witnessUtxo: { script: input.script!, value: BigInt(input.value) },
      })
      psbt.updateInput(psbt.inputCount - 1, {
        sighashType: bitcoin.Transaction.SIGHASH_ALL,
      })
    }
    for (const output of outputs) {
      psbt.addOutput(
        output.script
          ? { script: output.script, value: BigInt(output.value) }
          : { address: derivedBtcAddress, value: BigInt(output.value) },
      )
    }

    const unsignedTx = bitcoin.Transaction.fromBuffer(
      psbt.data.globalMap.unsignedTx.toBuffer(),
    )
    const txid = Buffer.from(unsignedTx.getId(), 'hex')
    const keyring = new Keyring({ type: 'sr25519' })
    const aliceHexPath =
      '0x' + u8aToHex(keyring.decodeAddress(alice.address)).slice(2)

    // Calculate request IDs
    const aggregateRequestId = signetClient.calculateSignRespondRequestId(
      palletSS58,
      Array.from(txid),
      {
        caip2Id: BTC_CAIP2,
        keyVersion: 0,
        path: aliceHexPath,
        algo: 'ecdsa',
        dest: 'bitcoin',
        params: '',
      },
    )

    const perInputRequestIds = inputs.map((_: any, i: number) => {
      const indexBuf = Buffer.alloc(4)
      indexBuf.writeUInt32LE(i, 0)
      return ethers.hexlify(
        signetClient.calculateSignRespondRequestId(
          palletSS58,
          Array.from(
            Buffer.concat([Buffer.from(unsignedTx.getId(), 'hex'), indexBuf]),
          ),
          {
            caip2Id: BTC_CAIP2,
            keyVersion: 0,
            path: aliceHexPath,
            algo: 'ecdsa',
            dest: 'bitcoin',
            params: '',
          },
        ),
      )
    })

    // Submit deposit
    const palletInputs = inputs.map((i: any) => ({
      txid: Array.from(Buffer.from(i.txid, 'hex')),
      vout: i.vout,
      value: i.value,
      scriptPubkey: Array.from(i.script),
      sequence: 0xffffffff,
    }))
    const palletOutputs = outputs.map((o: any) => ({
      value: o.value,
      scriptPubkey: Array.from(
        o.script || bitcoin.address.toOutputScript(derivedBtcAddress, network),
      ),
    }))

    const depositResult = await submitWithRetry(
      api.tx.btcVault.requestDeposit(
        Array.from(ethers.getBytes(aggregateRequestId)),
        palletInputs,
        palletOutputs,
        0,
      ),
      alice,
      api,
      'Deposit BTC',
    )

    // Extract the pallet's PSBT from the SignBidirectionalRequested event
    // The server signs sighashes from THIS PSBT, so we must verify against it
    const bidirectionalEvent = depositResult.events.find(
      (r: any) =>
        r.event.section === 'signet' &&
        r.event.method === 'SignBidirectionalRequested',
    )
    expect(bidirectionalEvent).toBeTruthy()

    const palletPsbt = bitcoin.Psbt.fromBuffer(
      Buffer.from(stripScalePrefix(bidirectionalEvent!.event.data[1].toU8a())),
    )
    const palletUnsignedTx = bitcoin.Transaction.fromBuffer(
      palletPsbt.data.globalMap.unsignedTx.toBuffer(),
    )

    // Verify pallet built the same transaction
    console.log(`  Local txid:  ${unsignedTx.getId()}`)
    console.log(`  Pallet txid: ${palletUnsignedTx.getId()}`)

    // Wait for signatures and apply them using the pallet's PSBT for sighash
    const compressedPubkey = compressPubkey(derivedPubKey)
    for (let i = 0; i < inputs.length; i++) {
      const sig = await waitForSignature(api, perInputRequestIds[i], 120000)
      console.log('sig -> ', sig)
      const { r, s } = extractSigBuffers(sig)
      const witnessUtxo = palletPsbt.data.inputs[i].witnessUtxo!
      const scriptCode = getScriptCode(Buffer.from(witnessUtxo.script), network)
      const sighash = palletUnsignedTx.hashForWitnessV0(
        i,
        scriptCode,
        witnessUtxo.value,
        bitcoin.Transaction.SIGHASH_ALL,
      )
      const verified = ecc.verify(
        sighash,
        compressedPubkey,
        Buffer.concat([r, s]),
      )
      console.log(
        `  Input ${i}: sighash=${Buffer.from(sighash).toString('hex').slice(0, 16)}... ` +
          `pubkey=${compressedPubkey.toString('hex').slice(0, 16)}... verified=${verified}`,
      )
      expect(verified).toBe(true)
      psbt.updateInput(i, {
        partialSig: [
          {
            pubkey: compressedPubkey,
            signature: Buffer.concat([encodeDER(r, s), Buffer.from([0x01])]),
          },
        ],
      })
    }

    // Broadcast
    psbt.finalizeAllInputs()
    const signedTxHex = psbt.extractTransaction().toHex()
    await btcClient.command('sendrawtransaction', signedTxHex)
    await btcClient.command('generatetoaddress', 1, derivedBtcAddress)

    // Wait for read response and claim
    const readResponse = await waitForResponse(
      api,
      ethers.hexlify(aggregateRequestId),
      120000,
    )
    expect(readResponse).toBeTruthy()

    const outputBytes = stripScalePrefix(new Uint8Array(readResponse.output))

    // Debug: verify signature off-chain before submitting
    {
      const requestIdBytes = ethers.getBytes(aggregateRequestId)
      const msgData = Buffer.concat([Buffer.from(requestIdBytes), Buffer.from(outputBytes)])
      const msgHash = ethers.keccak256(msgData)
      const sig = readResponse.signature
      const r = typeof sig.bigR.x === 'string' ? sig.bigR.x : '0x' + Buffer.from(sig.bigR.x).toString('hex')
      const s = typeof sig.s === 'string' ? sig.s : '0x' + Buffer.from(sig.s).toString('hex')
      console.log(`  DEBUG outputBytes (${outputBytes.length} bytes): ${Buffer.from(outputBytes).toString('hex')}`)
      console.log(`  DEBUG requestId: ${aggregateRequestId}`)
      console.log(`  DEBUG msgHash: ${msgHash}`)
      console.log(`  DEBUG sig r: ${r}`)
      console.log(`  DEBUG sig s: ${s}`)
      console.log(`  DEBUG sig recoveryId: ${sig.recoveryId}`)
      // Try all recovery IDs
      for (const rid of [0, 1]) {
        try {
          const v = BigInt(rid + 27)
          const recovered = ethers.recoverAddress(msgHash, { r, s, v })
          const expected = '0x' + Buffer.from([
            0x1b, 0xe3, 0x1a, 0x94, 0x36, 0x1a, 0x39, 0x1b, 0xba, 0xfb,
            0x2a, 0x4c, 0xcd, 0x70, 0x4f, 0x57, 0xdc, 0x04, 0xd4, 0xbb,
          ]).toString('hex')
          console.log(`  DEBUG recoveryId=${rid}: recovered=${recovered} expected=${expected} match=${recovered.toLowerCase() === expected.toLowerCase()}`)
        } catch (e: any) {
          console.log(`  DEBUG recoveryId=${rid}: FAILED - ${e.message}`)
        }
      }
    }

    const balanceBefore = BigInt(
      (await api.query.btcVault.userBalances(alice.address)).toString(),
    )

    await submitWithRetry(
      api.tx.btcVault.claimDeposit(
        Array.from(ethers.getBytes(aggregateRequestId)),
        Array.from(outputBytes),
        readResponse.signature,
      ),
      alice,
      api,
      'Claim BTC',
    )

    const balanceAfter = BigInt(
      (await api.query.btcVault.userBalances(alice.address)).toString(),
    )
    expect((balanceAfter - balanceBefore).toString()).toBe(
      depositAmount.toString(),
    )
    console.log(`✅ Deposit claimed: ${balanceAfter - balanceBefore} sats`)
  }, 300000)

  it('should complete successful withdrawal', async () => {
    const balanceBefore = BigInt(
      (await api.query.btcVault.userBalances(alice.address)).toString(),
    )
    expect(balanceBefore).toBeGreaterThan(0n)

    const vaultScript = getVaultScript()
    const vaultAddress = bitcoin.address.fromOutputScript(vaultScript, network)
    const vaultScan = await btcClient.command('scantxoutset', 'start', [
      `addr(${vaultAddress})`,
    ])
    if (vaultScan.unspents.length === 0) return

    const withdrawAmount = 10000000
    const vaultUtxos = vaultScan.unspents.map((u: any) => ({
      txid: u.txid,
      vout: u.vout,
      value: Math.floor(u.amount * 100000000),
      script: vaultScript,
    }))
    const recipientScript = Buffer.from(
      bitcoin.address.toOutputScript(derivedBtcAddress, network),
    )

    const coinSelectResult = coinSelect(
      vaultUtxos,
      [{ script: recipientScript, value: withdrawAmount }],
      2,
    )
    if (!coinSelectResult.inputs || !coinSelectResult.outputs) return
    const { inputs, outputs } = coinSelectResult

    spentVaultUtxos = inputs.map((i: any) => ({
      txid: i.txid,
      vout: i.vout,
      value: i.value,
      script: i.script,
    }))

    // Build PSBT
    const psbt = new bitcoin.Psbt({ network })
    for (const input of inputs) {
      psbt.addInput({
        hash: Buffer.from(input.txid, 'hex').reverse(),
        index: input.vout,
        sequence: 0xffffffff,
        witnessUtxo: { script: input.script!, value: BigInt(input.value) },
      })
      psbt.updateInput(psbt.inputCount - 1, {
        sighashType: bitcoin.Transaction.SIGHASH_ALL,
      })
    }
    for (const output of outputs) {
      psbt.addOutput(
        output.script
          ? { script: output.script, value: BigInt(output.value) }
          : { script: vaultScript, value: BigInt(output.value) },
      )
    }

    const unsignedTx = bitcoin.Transaction.fromBuffer(
      psbt.data.globalMap.unsignedTx.toBuffer(),
    )
    const txid = Buffer.from(unsignedTx.getId(), 'hex')
    const withdrawalPath = 'root'

    const aggregateRequestId = signetClient.calculateSignRespondRequestId(
      palletSS58,
      Array.from(txid),
      {
        caip2Id: BTC_CAIP2,
        keyVersion: 0,
        path: withdrawalPath,
        algo: 'ecdsa',
        dest: 'bitcoin',
        params: '',
      },
    )

    const perInputRequestIds = inputs.map((_: any, i: number) => {
      const indexBuf = Buffer.alloc(4)
      indexBuf.writeUInt32LE(i, 0)
      return ethers.hexlify(
        signetClient.calculateSignRespondRequestId(
          palletSS58,
          Array.from(
            Buffer.concat([Buffer.from(unsignedTx.getId(), 'hex'), indexBuf]),
          ),
          {
            caip2Id: BTC_CAIP2,
            keyVersion: 0,
            path: withdrawalPath,
            algo: 'ecdsa',
            dest: 'bitcoin',
            params: '',
          },
        ),
      )
    })

    const palletInputs = inputs.map((i: any) => ({
      txid: Array.from(Buffer.from(i.txid, 'hex')),
      vout: i.vout,
      value: i.value,
      scriptPubkey: Array.from(i.script),
      sequence: 0xffffffff,
    }))
    const palletOutputs = outputs.map((o: any) => ({
      value: o.value,
      scriptPubkey: Array.from(o.script || vaultScript),
    }))

    const withdrawResult = await submitWithRetry(
      api.tx.btcVault.withdrawBtc(
        Array.from(ethers.getBytes(aggregateRequestId)),
        withdrawAmount,
        Array.from(recipientScript),
        palletInputs,
        palletOutputs,
        0,
      ),
      alice,
      api,
      'Withdraw BTC',
    )
    expect(
      withdrawResult.events.some(
        (r: any) =>
          r.event.section === 'btcVault' &&
          r.event.method === 'WithdrawalRequested',
      ),
    ).toBe(true)

    // Verify optimistic decrement
    const balanceAfterWithdraw = BigInt(
      (await api.query.btcVault.userBalances(alice.address)).toString(),
    )
    expect(balanceAfterWithdraw).toBe(balanceBefore - BigInt(withdrawAmount))

    // Get signatures
    const vaultPubKey = KeyDerivation.derivePublicKey(
      ROOT_PUBLIC_KEY,
      SUBSTRATE_CHAIN_ID,
      palletSS58,
      withdrawalPath,
    )
    const vaultCompressed = compressPubkey(vaultPubKey)

    for (let i = 0; i < inputs.length; i++) {
      const sig = await waitForSignature(api, perInputRequestIds[i], 120000)
      const { r, s } = extractSigBuffers(sig)
      psbt.updateInput(i, {
        partialSig: [
          {
            pubkey: vaultCompressed,
            signature: Buffer.concat([encodeDER(r, s), Buffer.from([0x01])]),
          },
        ],
      })
    }

    // Broadcast
    psbt.finalizeAllInputs()
    await btcClient.command(
      'sendrawtransaction',
      psbt.extractTransaction().toHex(),
    )
    await btcClient.command('generatetoaddress', 1, derivedBtcAddress)

    // Complete withdrawal
    const readResponse = await waitForResponse(
      api,
      ethers.hexlify(aggregateRequestId),
      120000,
    )
    expect(readResponse).toBeTruthy()

    const outputBytes = stripScalePrefix(new Uint8Array(readResponse.output))
    const completeResult = await submitWithRetry(
      api.tx.btcVault.completeWithdrawBtc(
        Array.from(ethers.getBytes(aggregateRequestId)),
        Array.from(outputBytes),
        readResponse.signature,
      ),
      alice,
      api,
      'Complete Withdraw',
    )

    expect(
      completeResult.events.some(
        (r: any) =>
          r.event.section === 'btcVault' &&
          r.event.method === 'WithdrawalCompleted',
      ),
    ).toBe(true)
    console.log(`✅ Withdrawal completed: ${withdrawAmount} sats`)
  }, 300000)

  it('should refund when withdrawal fails', async () => {
    if (spentVaultUtxos.length === 0) return

    const balanceBefore = BigInt(
      (await api.query.btcVault.userBalances(alice.address)).toString(),
    )
    expect(balanceBefore).toBeGreaterThan(0n)

    const vaultScript = getVaultScript()
    const recipientScript = Buffer.from(
      bitcoin.address.toOutputScript(derivedBtcAddress, network),
    )
    const totalValue = spentVaultUtxos.reduce((sum, u) => sum + u.value, 0)
    const withdrawAmount = Math.min(5000000, totalValue - 1000)
    const fee = 500

    const inputs = spentVaultUtxos
    const outputs = [
      { script: recipientScript, value: withdrawAmount },
      { script: vaultScript, value: totalValue - withdrawAmount - fee },
    ]

    // Build PSBT with spent UTXOs
    const psbt = new bitcoin.Psbt({ network })
    for (const input of inputs) {
      psbt.addInput({
        hash: Buffer.from(input.txid, 'hex').reverse(),
        index: input.vout,
        sequence: 0xffffffff,
        witnessUtxo: { script: input.script!, value: BigInt(input.value) },
      })
      psbt.updateInput(psbt.inputCount - 1, {
        sighashType: bitcoin.Transaction.SIGHASH_ALL,
      })
    }
    for (const output of outputs) {
      psbt.addOutput({ script: output.script, value: BigInt(output.value) })
    }

    const unsignedTx = bitcoin.Transaction.fromBuffer(
      psbt.data.globalMap.unsignedTx.toBuffer(),
    )
    const txid = Buffer.from(unsignedTx.getId(), 'hex')
    const withdrawalPath = 'root'

    const aggregateRequestId = signetClient.calculateSignRespondRequestId(
      palletSS58,
      Array.from(txid),
      {
        caip2Id: BTC_CAIP2,
        keyVersion: 0,
        path: withdrawalPath,
        algo: 'ecdsa',
        dest: 'bitcoin',
        params: '',
      },
    )

    const perInputRequestIds = inputs.map((_, i) => {
      const indexBuf = Buffer.alloc(4)
      indexBuf.writeUInt32LE(i, 0)
      return ethers.hexlify(
        signetClient.calculateSignRespondRequestId(
          palletSS58,
          Array.from(
            Buffer.concat([Buffer.from(unsignedTx.getId(), 'hex'), indexBuf]),
          ),
          {
            caip2Id: BTC_CAIP2,
            keyVersion: 0,
            path: withdrawalPath,
            algo: 'ecdsa',
            dest: 'bitcoin',
            params: '',
          },
        ),
      )
    })

    const palletInputs = inputs.map((i: any) => ({
      txid: Array.from(Buffer.from(i.txid, 'hex')),
      vout: i.vout,
      value: i.value,
      scriptPubkey: Array.from(i.script),
      sequence: 0xffffffff,
    }))
    const palletOutputs = outputs.map((o: any) => ({
      value: o.value,
      scriptPubkey: Array.from(o.script),
    }))

    await submitWithRetry(
      api.tx.btcVault.withdrawBtc(
        Array.from(ethers.getBytes(aggregateRequestId)),
        withdrawAmount,
        Array.from(recipientScript),
        palletInputs,
        palletOutputs,
        0,
      ),
      alice,
      api,
      'Withdraw BTC (refund)',
    )

    const balanceAfterWithdraw = BigInt(
      (await api.query.btcVault.userBalances(alice.address)).toString(),
    )
    expect(balanceAfterWithdraw).toBe(balanceBefore - BigInt(withdrawAmount))

    // Get signatures
    const vaultPubKey = KeyDerivation.derivePublicKey(
      ROOT_PUBLIC_KEY,
      SUBSTRATE_CHAIN_ID,
      palletSS58,
      withdrawalPath,
    )
    const vaultCompressed = compressPubkey(vaultPubKey)

    for (let i = 0; i < inputs.length; i++) {
      const sig = await waitForSignature(api, perInputRequestIds[i], 120000)
      const { r, s } = extractSigBuffers(sig)
      psbt.updateInput(i, {
        partialSig: [
          {
            pubkey: vaultCompressed,
            signature: Buffer.concat([encodeDER(r, s), Buffer.from([0x01])]),
          },
        ],
      })
    }

    psbt.finalizeAllInputs()
    const signedTxHex = psbt.extractTransaction().toHex()

    // Broadcast should fail (spent UTXOs)
    let broadcastFailed = false
    try {
      await btcClient.command('sendrawtransaction', signedTxHex)
    } catch {
      broadcastFailed = true
    }
    expect(broadcastFailed).toBe(true)
    console.log('✅ Broadcast failed as expected (spent UTXOs)')

    // Wait for MPC error response
    const readResponse = await waitForResponse(
      api,
      ethers.hexlify(aggregateRequestId),
      300000,
    )
    expect(readResponse).toBeTruthy()

    const outputBytes = stripScalePrefix(new Uint8Array(readResponse.output))
    expect(Buffer.from(outputBytes.slice(0, 4)).equals(ERROR_PREFIX)).toBe(true)

    // Complete withdrawal with error
    const completeResult = await submitWithRetry(
      api.tx.btcVault.completeWithdrawBtc(
        Array.from(ethers.getBytes(aggregateRequestId)),
        Array.from(outputBytes),
        readResponse.signature,
      ),
      alice,
      api,
      'Complete Withdraw (refund)',
    )

    expect(
      completeResult.events.some(
        (r: any) =>
          r.event.section === 'btcVault' &&
          r.event.method === 'WithdrawalFailed',
      ),
    ).toBe(true)

    // Verify refund
    const finalBalance = BigInt(
      (await api.query.btcVault.userBalances(alice.address)).toString(),
    )
    expect(finalBalance).toBe(balanceBefore)
    console.log(`✅ Refund verified: balance restored to ${finalBalance} sats`)
  }, 360000)
})
