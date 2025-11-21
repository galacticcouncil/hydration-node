import * as anchor from '@coral-xyz/anchor'
import { Program } from '@coral-xyz/anchor'
import { Connection } from '@solana/web3.js'
import { ChainSignaturesProject } from './types/chain_signatures_project'
import IDL from './idl/chain_signatures_project.json'
import * as path from 'path'
import * as dotenv from 'dotenv'
import { CryptoUtils } from './crypto-utils'
import { CONFIG } from './config'
import { PendingTransaction, TransactionOutput } from './types'
import { RequestIdGenerator } from './request-id-generator'
import { TransactionProcessor } from './transaction-processor'
import { EthereumMonitor } from './ethereum-monitor'
import { OutputSerializer } from './output-serializer'
import { SolanaUtils } from './solana-utils'
import { CpiEventParser } from './cpi-event-parser'
import { SubstrateMonitor } from './substrate-monitor'
import { ethers } from 'ethers'
import * as borsh from 'borsh'

dotenv.config({ path: path.resolve(__dirname, '../../.env') })

const pendingTransactions = new Map<string, PendingTransaction>()

class ChainSignatureServer {
  private connection: Connection
  private wallet: anchor.Wallet
  private provider: anchor.AnchorProvider
  private program: Program<ChainSignaturesProject>
  private pollCounter = 0
  private cpiSubscriptionId: number | null = null
  private substrateMonitor: SubstrateMonitor

  constructor() {
    this.connection = new Connection(
      process.env.RPC_URL || 'https://api.devnet.solana.com',
      'confirmed'
    )

    this.wallet = new anchor.Wallet(SolanaUtils.loadKeypair())
    this.provider = new anchor.AnchorProvider(this.connection, this.wallet, {
      commitment: 'confirmed',
    })
    anchor.setProvider(this.provider)

    this.program = new Program<ChainSignaturesProject>(IDL, this.provider)
    this.substrateMonitor = new SubstrateMonitor(
      process.env.SUBSTRATE_WS_URL || 'ws://localhost:9944'
    )
  }

  async start() {
    console.log('🚀 Chain Signature Server Starting...')
    console.log('  👛 Solana wallet:', this.wallet.publicKey.toString())
    console.log(
      '  🔑 Ethereum signer:',
      process.env.PRIVATE_KEY_TESTNET!.slice(0, 6) + '...'
    )
    console.log('  📍 Watching program:', this.program.programId.toString())
    console.log('\n📡 Server capabilities:')
    console.log('  ✅ Sign transactions when requested')
    console.log('  ✅ Monitor Ethereum for executed transactions')
    console.log('  ✅ Return transaction outputs when detected')
    console.log('  ✅ Support for CPI events (SignatureRequestedEvent)\n')

    await this.connectToSubstrate()
    // Check wallet balance
    const balance = await this.connection.getBalance(this.wallet.publicKey)
    console.log('  💰 Wallet balance:', balance / 1e9, 'SOL')
    if (balance < 0.01 * 1e9) {
      console.warn(
        '  ⚠️  Low balance! You may need more SOL to respond to signature requests.'
      )
    }

    this.startTransactionMonitor()
    this.setupEventListeners()

    console.log('\n✅ Server is running. Press Ctrl+C to exit.\n')
  }

  private async connectToSubstrate() {
    try {
      await this.substrateMonitor.connect()

      // Set up Substrate event handlers
      await this.substrateMonitor.subscribeToEvents({
        onSignatureRequested: async (event) => {
          console.log('\n📝 New Substrate SignatureRequested')
          console.log('  👤 Sender:', event.sender)
          console.log(
            '  📊 Payload:',
            Buffer.from(event.payload).toString('hex')
          )
          console.log('  🔗 Chain ID:', event.chainId)

          try {
            await this.handleSubstrateSignatureRequest(event)
          } catch (error) {
            console.error(
              '❌ Error processing Substrate signature request:',
              error
            )
          }
        },

        onSignRespondRequested: async (event) => {
          console.log('\n📨 New Substrate SignRespondRequested')
          console.log('  👤 Sender:', event.sender)
          console.log('  🔗 Chain ID:', event.slip44ChainId)

          try {
            await this.handleSubstrateSignRespondRequest(event)
          } catch (error) {
            console.error(
              '❌ Error processing Substrate sign-respond request:',
              error
            )
          }
        },

        onReadResponded: async (event) => {
          console.log('\n📖 Substrate ReadResponded received')
          console.log('  🔑 Request ID:', event.requestId)
          console.log('  👤 Responder:', event.responder)
        },
      })
    } catch (error) {
      console.error('❌ Failed to connect to Substrate:', error)
    }
  }

  private async handleSubstrateSignatureRequest(event: any) {
    console.log(event, '<< subs event')
    const privateKey = process.env.PRIVATE_KEY_TESTNET!
    const wallet = new ethers.Wallet(privateKey)
    console.log("Server's root public key:", wallet.signingKey.publicKey)
    const path = Buffer.from(event.path.slice(2), 'hex').toString()
    const algo = Buffer.from(event.algo.slice(2), 'hex').toString()
    const dest =
      event.dest === '0x'
        ? ''
        : Buffer.from(event.dest.slice(2), 'hex').toString()
    const params = Buffer.from(event.params.slice(2), 'hex').toString()
    const chainId = Buffer.from(event.chainId.slice(2), 'hex').toString()

    // Generate request ID (you'll need to adapt your RequestIdGenerator)
    console.log('  Decoded values:')
    console.log('    Path:', path)
    console.log('    Algo:', algo)
    console.log('    ChainId:', chainId)
    console.log('    Params:', params)

    // Generate request ID with all decoded values
    const requestId = RequestIdGenerator.generateRequestIdStringChainId(
      event.sender,
      Array.from(event.payload),
      path,
      event.keyVersion,
      chainId,
      algo,
      dest,
      params
    )

    console.log('  🔑 Request ID:', requestId)

    // Derive signing key
    const derivedPrivateKey = await CryptoUtils.deriveSigningKeyWithChainId(
      path,
      event.sender,
      process.env.PRIVATE_KEY_TESTNET!,
      chainId
    )

    // Sign the payload
    const signature = await CryptoUtils.signMessage(
      event.payload,
      derivedPrivateKey
    )

    // Send response to Substrate
    await this.substrateMonitor.sendSignatureResponse(
      Buffer.from(requestId.slice(2), 'hex'),
      signature,
      event.sender
    )

    console.log('  ✅ Signature sent to Substrate!')
  }

  private async handleSubstrateSignRespondRequest(event: any) {
    console.log(event, '<< subs event')
    const path = Buffer.from(event.path.slice(2), 'hex').toString()
    const algo = Buffer.from(event.algo.slice(2), 'hex').toString()
    const dest =
      event.dest === '0x'
        ? ''
        : Buffer.from(event.dest.slice(2), 'hex').toString()
    const params = Buffer.from(event.params.slice(2), 'hex').toString()

    // Generate request ID (you'll need to adapt your RequestIdGenerator)
    console.log('  Decoded values:')
    console.log('    Path:', path)
    console.log('    Algo:', algo)
    console.log('    Params:', params)
    // Similar to handleSignRespondRequest but for Substrate
    const requestId = RequestIdGenerator.generateSignRespondRequestId(
      event.sender,
      Array.from(event.transactionData),
      event.slip44ChainId, // This is a number, not a hex string
      event.keyVersion,
      path,
      algo,
      dest,
      params
    )

    console.log('  🔑 Request ID:', requestId)

    // Derive signing key
    const derivedPrivateKey = await CryptoUtils.deriveSigningKeyWithChainId(
      path,
      event.sender,
      process.env.PRIVATE_KEY_TESTNET!,
      'polkadot:2034'
    )

    const result = await TransactionProcessor.processTransactionForSigning(
      event.transactionData,
      derivedPrivateKey,
      event.slip44ChainId
    )

    // Send signature response to Substrate
    await this.substrateMonitor.sendSignatureResponse(
      Buffer.from(requestId.slice(2), 'hex'),
      result.signature,
      event.sender
    )

    // Add to pending transactions for monitoring
    pendingTransactions.set(result.signedTxHash, {
      txHash: result.signedTxHash,
      requestId,
      chainId: event.slip44ChainId,
      explorerDeserializationFormat: event.explorerDeserializationFormat,
      explorerDeserializationSchema: event.explorerDeserializationSchema,
      callbackSerializationFormat: event.callbackSerializationFormat,
      callbackSerializationSchema: event.callbackSerializationSchema,
      sender: event.sender,
      path: event.path,
      fromAddress: result.fromAddress,
      nonce: result.nonce,
      checkCount: 0,
      source: 'polkadot', // Add this to track the source
    })

    console.log('  ✅ Signature sent to Substrate!')
    console.log('  👀 Now monitoring for execution...')
  }

  private startTransactionMonitor() {
    setInterval(async () => {
      this.pollCounter++

      if (pendingTransactions.size > 0 && this.pollCounter % 12 === 1) {
        console.log(
          `\n📊 Monitoring ${pendingTransactions.size} pending transaction(s)...`
        )
      }

      for (const [txHash, txInfo] of pendingTransactions.entries()) {
        // CHANGE 4: Exponential backoff - check less frequently as time passes
        if (txInfo.checkCount > 0) {
          // Skip checks based on how many times we've already checked
          // 0-5 checks: every 5s
          // 6-10 checks: every 10s
          // 11-20 checks: every 30s
          // 20+ checks: every 60s
          let skipFactor = 1
          if (txInfo.checkCount > 20) skipFactor = 12
          else if (txInfo.checkCount > 10) skipFactor = 6
          else if (txInfo.checkCount > 5) skipFactor = 2

          if (this.pollCounter % skipFactor !== 0) {
            continue // Skip this check
          }
        }

        try {
          const result = await EthereumMonitor.waitForTransactionAndGetOutput(
            txHash,
            txInfo.chainId,
            txInfo.explorerDeserializationFormat,
            txInfo.explorerDeserializationSchema,
            txInfo.fromAddress,
            txInfo.nonce
          )

          // Increment check count
          txInfo.checkCount++

          switch (result.status) {
            case 'pending':
              // Just increment count, continue polling
              break

            case 'success':
              await this.handleCompletedTransaction(txHash, txInfo, {
                success: result.success!,
                output: result.output,
              })
              pendingTransactions.delete(txHash)
              break

            case 'error':
              // Only for reverted/replaced - send signed error
              await this.handleFailedTransaction(txHash, txInfo)
              pendingTransactions.delete(txHash)
              break

            case 'fatal_error':
              // Just remove from map, don't send signed error
              console.error(`Fatal error for ${txHash}:`, result.reason)
              pendingTransactions.delete(txHash)
              break
          }
        } catch (error: any) {
          if (
            error.message &&
            (error.message.includes('Modulus not supported') ||
              error.message.includes('Failed to parse SOLANA_PRIVATE_KEY') ||
              error.message.includes('Failed to load keypair'))
          ) {
            console.error(`Infrastructure error for ${txHash}:`, error.message)
            pendingTransactions.delete(txHash)
          } else {
            console.error(`Unexpected error polling ${txHash}:`, error)
            txInfo.checkCount++ // Still increment count
          }
        }
      }
    }, CONFIG.POLL_INTERVAL_MS)
  }

  private async handleCompletedTransaction(
    txHash: string,
    txInfo: PendingTransaction,
    result: TransactionOutput
  ) {
    console.log(`\n🎉 Transaction ${txHash} completed!`)
    console.log(`  ✅ Success: ${result.success}`)
    console.log(`  📊 Output:`, JSON.stringify(result.output, null, 2))

    const serializedOutput = await OutputSerializer.serialize(
      result.output,
      txInfo.callbackSerializationFormat,
      txInfo.callbackSerializationSchema
    )

    const requestIdBytes = Buffer.from(txInfo.requestId.slice(2), 'hex')
    const messageHash = CryptoUtils.hashMessage(
      requestIdBytes,
      serializedOutput
    )

    console.log('\n🔍 Signature Debug:')
    console.log('  Request ID:', txInfo.requestId)
    console.log(
      '  Serialized output (hex):',
      Buffer.from(serializedOutput).toString('hex')
    )
    console.log('  Message hash:', ethers.hexlify(messageHash))
    console.log(
      '  Signing with root key address:',
      new ethers.Wallet(process.env.PRIVATE_KEY_TESTNET!).address
    )

    const signature = await CryptoUtils.signMessage(
      messageHash,
      process.env.PRIVATE_KEY_TESTNET!
    )

    try {
      // Check source and send to appropriate chain
      if (txInfo.source === 'polkadot') {
        await this.substrateMonitor.sendReadResponse(
          requestIdBytes,
          serializedOutput,
          signature,
          txInfo.sender
        )
        console.log('  ✅ Read response sent!')
      } else {
        const tx = await this.program.methods
          .readRespond(
            Array.from(requestIdBytes),
            Buffer.from(serializedOutput),
            signature
          )
          .accounts({
            responder: this.wallet.publicKey,
          })
          .rpc()

        console.log('  ✅ Read response sent!')
        console.log('  🔗 Solana tx:', tx)
      }

      pendingTransactions.delete(txHash)
    } catch (error) {
      console.error('  ❌ Error sending read response:', error)
    }
  }

  private async handleFailedTransaction(
    txHash: string,
    txInfo: PendingTransaction
  ) {
    console.log(`\n❌ Transaction ${txHash} failed`)

    try {
      // Magic prefix to identify error responses
      const MAGIC_ERROR_PREFIX = Buffer.from([0xde, 0xad, 0xbe, 0xef])

      let errorData: Buffer
      if (txInfo.callbackSerializationFormat === 0) {
        // Borsh - add magic prefix
        const errorSchema = { struct: { error: 'bool' } }
        const borshData = borsh.serialize(errorSchema as any, { error: true })
        errorData = Buffer.concat([MAGIC_ERROR_PREFIX, borshData])
      } else {
        // ABI - add magic prefix
        const encoded = ethers.AbiCoder.defaultAbiCoder().encode(
          ['bool'],
          [true]
        )
        errorData = Buffer.concat([
          MAGIC_ERROR_PREFIX,
          ethers.getBytes(encoded),
        ])
      }

      const serializedOutput = new Uint8Array(errorData)

      const requestIdBytes = Buffer.from(txInfo.requestId.slice(2), 'hex')
      const messageHash = CryptoUtils.hashMessage(
        requestIdBytes,
        serializedOutput
      )

      const signature = await CryptoUtils.signMessage(
        messageHash,
        process.env.PRIVATE_KEY_TESTNET!
      )

      const tx = await this.program.methods
        .readRespond(
          Array.from(requestIdBytes),
          Buffer.from(serializedOutput),
          signature
        )
        .accounts({
          responder: this.wallet.publicKey,
        })
        .rpc()

      console.log('  ✅ Error response sent!')
      console.log('  🔗 Solana tx:', tx)
    } catch (error) {
      console.error('  ❌ Error sending error response:', error)
    }
  }

  private setupEventListeners() {
    // CPI event listener for SignatureRequestedEvent only
    const cpiEventHandlers = new Map<
      string,
      (event: any, slot: number) => Promise<void>
    >()

    // Standard event listeners for non-CPI events
    cpiEventHandlers.set('signRespondRequestedEvent', async (event, slot) => {
      console.log('\n📨 New SignRespondRequestedEvent')
      console.log('  📍 Slot:', slot)
      console.log('  👤 Sender:', event.sender.toString())
      console.log('  🔗 Chain ID:', event.slip44ChainId)
      console.log('  📂 Path:', event.path)

      this.logSchemaInfo(
        'Explorer',
        event.explorerDeserializationFormat,
        event.explorerDeserializationSchema
      )
      this.logSchemaInfo(
        'Callback',
        event.callbackSerializationFormat,
        event.callbackSerializationSchema
      )

      try {
        await this.handleSignRespondRequest(event)
      } catch (error) {
        console.error('❌ Error processing transaction:', error)
      }
    })

    // Note: Anchor's BorshEventCoder returns event names in camelCase
    cpiEventHandlers.set('signatureRequestedEvent', async (event, slot) => {
      console.log('\n📝 New SignatureRequestedEvent (CPI)')
      console.log('  📍 Slot:', slot)
      console.log('  👤 Sender:', event.sender.toString())
      console.log('  📊 Payload:', Buffer.from(event.payload).toString('hex'))
      console.log('  📂 Path:', event.path)
      console.log('  🔢 Key version:', event.keyVersion)

      try {
        await this.handleSignatureRequest(event)
      } catch (error) {
        console.error('❌ Error sending signature response:', error)
      }
    })

    // Subscribe to CPI events
    this.cpiSubscriptionId = CpiEventParser.subscribeToCpiEvents(
      this.connection,
      this.program,
      cpiEventHandlers
    )

    // Standard event listener for ReadRespondedEvent
    this.program.addEventListener('readRespondedEvent', async (event, slot) => {
      console.log('\n📖 ReadRespondedEvent received')
      console.log('  📍 Slot:', slot)
      console.log(
        '  🔑 Request ID:',
        '0x' + Buffer.from(event.requestId).toString('hex')
      )
      console.log('  👤 Responder:', event.responder.toString())
    })
  }

  private logSchemaInfo(type: string, format: number, schema: any) {
    console.log(`\n  📋 ${type} Deserialization:`)
    console.log(`    Format: ${format === 0 ? 'Borsh' : 'AbiJson'}`)

    try {
      const schemaStr = new TextDecoder().decode(new Uint8Array(schema))
      if (schemaStr.trim()) {
        const parsed = JSON.parse(schemaStr)
        console.log(`    Schema:`, JSON.stringify(parsed, null, 2))
      }
    } catch {
      console.log(`    Schema: [Invalid or binary data]`)
    }
  }

  private async handleSignRespondRequest(event: any) {
    const requestId = RequestIdGenerator.generateSignRespondRequestId(
      event.sender.toString(),
      Array.from(event.transactionData),
      event.slip44ChainId,
      event.keyVersion,
      event.path,
      event.algo,
      event.dest,
      event.params
    )

    console.log('  🔑 Request ID:', requestId)

    const derivedPrivateKey = await CryptoUtils.deriveSigningKey(
      event.path,
      event.sender.toString(),
      process.env.PRIVATE_KEY_TESTNET!
    )

    const result = await TransactionProcessor.processTransactionForSigning(
      new Uint8Array(event.transactionData),
      derivedPrivateKey,
      event.slip44ChainId
    )

    console.log('\n✅ Transaction ready for submission')
    console.log('  🔗 Expected hash:', result.signedTxHash)

    const requestIdBytes = Array.from(Buffer.from(requestId.slice(2), 'hex'))
    const tx = await this.program.methods
      .respond([requestIdBytes], [result.signature])
      .accounts({
        responder: this.wallet.publicKey,
      })
      .rpc()

    console.log('  ✅ Signature sent!')
    console.log('  🔗 Solana tx:', tx)

    pendingTransactions.set(result.signedTxHash, {
      txHash: result.signedTxHash,
      requestId,
      chainId: event.slip44ChainId,
      explorerDeserializationFormat: event.explorerDeserializationFormat,
      explorerDeserializationSchema: event.explorerDeserializationSchema,
      callbackSerializationFormat: event.callbackSerializationFormat,
      callbackSerializationSchema: event.callbackSerializationSchema,
      sender: event.sender.toString(),
      path: event.path,
      fromAddress: result.fromAddress,
      nonce: result.nonce,
      checkCount: 0,
      source: 'solana',
    })

    console.log('  👀 Now monitoring for execution...')
  }

  private async handleSignatureRequest(event: any) {
    const requestId = RequestIdGenerator.generateRequestId(
      event.sender.toString(),
      Array.from(event.payload),
      event.path,
      event.keyVersion,
      0,
      event.algo,
      event.dest,
      event.params
    )

    console.log('  🔑 Request ID:', requestId)

    const derivedPrivateKey = await CryptoUtils.deriveSigningKey(
      event.path,
      event.sender.toString(),
      process.env.PRIVATE_KEY_TESTNET!
    )

    const signature = await CryptoUtils.signMessage(
      event.payload,
      derivedPrivateKey
    )

    const requestIdBytes = Array.from(Buffer.from(requestId.slice(2), 'hex'))
    const tx = await this.program.methods
      .respond([requestIdBytes], [signature])
      .accounts({
        responder: this.wallet.publicKey,
      })
      .rpc()

    console.log('  ✅ Signature sent!')
    console.log('  🔗 Solana tx:', tx)
  }

  async shutdown() {
    console.log('\n🛑 Shutting down...')
    if (this.cpiSubscriptionId !== null) {
      await this.connection.removeOnLogsListener(this.cpiSubscriptionId)
    }
    await this.substrateMonitor.disconnect()
    process.exit(0)
  }
}

async function main() {
  const server = new ChainSignatureServer()
  await server.start()

  // Handle graceful shutdown
  process.on('SIGINT', async () => {
    await server.shutdown()
  })
}

main().catch((err) => {
  console.error('❌ Fatal error:', err)
  process.exit(1)
})
