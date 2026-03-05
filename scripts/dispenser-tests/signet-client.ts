import { ApiPromise } from '@polkadot/api'
import { EventRecord } from '@polkadot/types/interfaces'
import { Vec } from '@polkadot/types'
import { u8aToHex } from '@polkadot/util'
import { ISubmittableResult } from '@polkadot/types/types'
import { ethers } from 'ethers'
import { keccak256, recoverAddress } from 'viem'
import { executeAsRoot } from './utils'

export class SignetClient {
  constructor(
    private api: ApiPromise,
    private signer: any,
  ) {}

  async ensureSignetInitialized(
    api: ApiPromise,
    signer: any,
    chainId: string,
  ) {
    // Skip if signet is already initialized (admin is set)
    const admin = await (api.query as any).signet.admin()
    if (admin && !admin.isNone) {
      console.log(`Signet already initialized (admin: ${admin.toString()}), skipping`)
      return
    }

    console.log('Signet not initialized, initializing via Root governance...')
    const chainIdBytes = Array.from(new TextEncoder().encode(chainId))
    const signetInitCall = api.tx.signet.initialize(
      signer.address,
      1_000_000_000_000n,
      chainIdBytes,
    )
    await executeAsRoot(
      api,
      signer,
      signetInitCall,
      'Initialize signet via Root',
    )
  }

  async requestSignature(payload: Uint8Array, params: any): Promise<void> {
    const tx = this.api.tx.signet.sign(
      Array.from(payload),
      params.keyVersion,
      params.path,
      params.algo,
      params.dest,
      params.params,
    )

    await new Promise<void>((resolve, reject) => {
      tx.signAndSend(this.signer, (result: any) => {
        const { status, dispatchError } = result
        if (dispatchError) {
          reject(dispatchError)
        } else if (status.isInBlock) {
          resolve()
        }
      }).catch(reject)
    })
  }

  async requestTransactionSignature(
    serializedTx: number[],
    params: any,
  ): Promise<void> {
    const tx = this.api.tx.signet.signRespond(
      serializedTx,
      params.slip44ChainId,
      params.keyVersion,
      params.path,
      params.algo || '',
      params.dest || '',
      params.params || '',
      params.schemas.explorer.format,
      Array.from(new TextEncoder().encode(params.schemas.explorer.schema)),
      params.schemas.callback.format,
      Array.from(new TextEncoder().encode(params.schemas.callback.schema)),
    )

    await tx.signAndSend(this.signer)
  }

  async waitForSignature(requestId: string, timeout: number): Promise<any> {
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

      const matchEvents = (events: any[]): any => {
        for (const record of events) {
          const { event } = record
          if (
            event.section === 'signet' &&
            event.method === 'SignatureResponded'
          ) {
            const [reqId, responder, signature] = event.data as any
            if (u8aToHex(reqId.toU8a()) === requestId) {
              return {
                responder: responder.toString(),
                signature: signature.toJSON(),
              }
            }
          }
        }
        return null
      }

      // 1. Subscribe to new events (future blocks)
      this.api.query.system
        .events((events: Vec<EventRecord>) => {
          const result = matchEvents(Array.from(events))
          if (result) done(result)
        })
        .then((unsub: any) => {
          unsubscribe = unsub
        })

      // 2. Scan recent blocks to catch events emitted before subscription started
      this.scanRecentBlocksForEvent(
        requestId,
        'SignatureResponded',
        30,
      ).then((result) => {
        if (result) done(result)
      }).catch(() => {})
    })
  }

  private async scanRecentBlocksForEvent(
    requestId: string,
    method: string,
    numBlocks: number,
  ): Promise<any> {
    try {
      const header = await this.api.rpc.chain.getHeader()
      const currentBlock = header.number.toNumber()
      const startBlock = Math.max(1, currentBlock - numBlocks)

      console.log(`Scanning blocks ${startBlock}..${currentBlock} for ${method}...`)

      for (let i = currentBlock; i >= startBlock; i--) {
        const hash = await this.api.rpc.chain.getBlockHash(i)
        const events = await this.api.query.system.events.at(hash) as any

        for (const record of events) {
          const { event } = record
          if (event.section === 'signet' && event.method === method) {
            const [reqId, responder, signature] = event.data as any
            if (u8aToHex(reqId.toU8a()) === requestId) {
              console.log(`Found ${method} in block ${i}`)
              return {
                responder: responder.toString(),
                signature: signature.toJSON(),
              }
            }
          }
        }
      }
    } catch (err) {
      console.warn(`Failed to scan recent blocks for ${method}:`, err)
    }
    return null
  }

  calculateRequestId(
    sender: string,
    payload: Uint8Array,
    params: any,
    chainId: string,
  ): string {
    const payloadHex = '0x' + Buffer.from(payload).toString('hex')
    const encoded = ethers.AbiCoder.defaultAbiCoder().encode(
      [
        'string',
        'bytes',
        'string',
        'uint32',
        'string',
        'string',
        'string',
        'string',
      ],
      [
        sender,
        payloadHex,
        params.path,
        params.keyVersion,
        chainId,
        params.algo,
        params.dest,
        params.params,
      ],
    )
    return ethers.keccak256(encoded)
  }

  calculateSignRespondRequestId(
    sender: string,
    txData: number[],
    params: any,
  ): string {
    const txHex = '0x' + Buffer.from(txData).toString('hex')
    const encoded = ethers.solidityPacked(
      [
        'string',
        'bytes',
        'string',
        'uint32',
        'string',
        'string',
        'string',
        'string',
      ],
      [
        sender,
        txHex,
        params.caip2_id,
        params.keyVersion,
        params.path,
        params.algo || '',
        params.dest || '',
        params.params || '',
      ],
    )
    return ethers.keccak256(encoded)
  }

  async verifySignature(
    payload: Uint8Array,
    signature: any,
    derivedPublicKey: string,
  ): Promise<boolean> {
    const r = signature.bigR.x.startsWith('0x')
      ? signature.bigR.x
      : `0x${signature.bigR.x}`
    const s = signature.s.startsWith('0x') ? signature.s : `0x${signature.s}`
    const v = BigInt(signature.recoveryId + 27)

    const recoveredAddress = await recoverAddress({
      hash: payload as any,
      signature: { r, s, v },
    })

    const expectedAddress =
      '0x' + keccak256(Buffer.from(derivedPublicKey.slice(4), 'hex')).slice(-40)

    console.log('       Recovered:', recoveredAddress)
    console.log('       Expected: ', expectedAddress)

    return recoveredAddress.toLowerCase() === expectedAddress.toLowerCase()
  }

  async verifyTransactionSignature(
    tx: ethers.Transaction,
    signature: any,
    derivedPublicKey: string,
  ): Promise<boolean> {
    const msgHash = ethers.keccak256(tx.unsignedSerialized)
    const r = signature.bigR.x.startsWith('0x')
      ? signature.bigR.x
      : `0x${signature.bigR.x}`
    const s = signature.s.startsWith('0x') ? signature.s : `0x${signature.s}`
    const v = BigInt(signature.recoveryId + 27)

    const recoveredAddress = await recoverAddress({
      hash: msgHash as `0x${string}`,
      signature: { r, s, v } as any,
    })

    const expectedAddress =
      '0x' + keccak256(Buffer.from(derivedPublicKey.slice(4), 'hex')).slice(-40)

    console.log('       Recovered:', recoveredAddress)
    console.log('       Expected: ', expectedAddress)

    return recoveredAddress.toLowerCase() === expectedAddress.toLowerCase()
  }
}
