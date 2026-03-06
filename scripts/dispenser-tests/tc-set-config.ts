/**
 * Set Signet and Dispenser on-chain configs.
 *
 * Modes (auto-detected):
 *   - Chopsticks: writes storage directly via dev_setStorage
 *   - Real network (lark/mainnet): creates a TC proposal via technicalCommittee.propose()
 *
 * Usage:
 *   # Uses SUBSTRATE_NETWORK from .env (defaults to chopsticks)
 *   npx ts-node tc-set-config.ts
 *
 *   # Override for lark — requires SURI of a TC member
 *   SUBSTRATE_NETWORK=lark SURI=//Alice npx ts-node tc-set-config.ts
 */

import { ApiPromise, WsProvider } from '@polkadot/api'
import { Keyring } from '@polkadot/keyring'
import { config } from 'dotenv'
import path from 'path'
import {
  SUBSTRATE_PRESETS,
  DEFAULT_FAUCET_ADDRESS,
  type SubstrateNetwork,
} from './networks'

config({ path: path.resolve(__dirname, '.env') })

// ---- Resolve substrate network ----

const networkName = (process.env.SUBSTRATE_NETWORK || 'chopsticks') as SubstrateNetwork
const preset = SUBSTRATE_PRESETS[networkName]
if (!preset) {
  console.error(`Unknown SUBSTRATE_NETWORK: ${networkName}`)
  process.exit(1)
}
const wsEndpoint = process.env.SUBSTRATE_WS_ENDPOINT || preset.wsEndpoint
const chainId = process.env.SUBSTRATE_CHAIN_ID || preset.chainId

// ---- Configuration values ----

const SIGNET_CONFIG = {
  signatureDeposit: 100_000_000_000n, // 0.1 HDX
  maxChainIdLength: 128,
  maxEvmDataLength: 100_000,
  chainId,
}

const DISPENSER_CONFIG = {
  faucetAddress: process.env.FAUCET_ADDRESS || DEFAULT_FAUCET_ADDRESS,
  minFaucetThreshold: 50_000_000_000_000_000n,   // 0.05 ETH
  minRequest: 0n,
  maxDispense: 1_000_000_000_000_000_000n,        // 1 ETH
  dispenserFee: 1_000_000_000_000n,               // 1 HDX (12 decimals)
  faucetBalanceWei: 10_000_000_000_000_000_000n,  // 10 ETH
}

// ---- Helpers ----

async function isChopsticks(api: ApiPromise): Promise<boolean> {
  try {
    await (api.rpc as any)('dev_newBlock', { count: 0 })
    return true
  } catch {
    return false
  }
}

function buildCalls(api: ApiPromise) {
  const chainIdBytes = Array.from(
    new TextEncoder().encode(SIGNET_CONFIG.chainId),
  )

  const signetCall = api.tx.signet.setConfig(
    SIGNET_CONFIG.signatureDeposit.toString(),
    SIGNET_CONFIG.maxChainIdLength,
    SIGNET_CONFIG.maxEvmDataLength,
    chainIdBytes,
  )

  const dispenserCall = (api.tx as any).ethDispenser.setConfig(
    DISPENSER_CONFIG.faucetAddress,
    DISPENSER_CONFIG.minFaucetThreshold.toString(),
    DISPENSER_CONFIG.minRequest.toString(),
    DISPENSER_CONFIG.maxDispense.toString(),
    DISPENSER_CONFIG.dispenserFee.toString(),
    DISPENSER_CONFIG.faucetBalanceWei.toString(),
  )

  return { signetCall, dispenserCall }
}

// ---- Chopsticks: write config storage directly ----

async function executeOnChopsticks(api: ApiPromise) {
  const chainIdHex =
    '0x' + Buffer.from(SIGNET_CONFIG.chainId).toString('hex')

  console.log('Writing Signet and Dispenser configs directly to storage...')

  await (api.rpc as any)('dev_setStorage', {
    Signet: {
      SignetConfig: {
        paused: false,
        signatureDeposit: SIGNET_CONFIG.signatureDeposit.toString(),
        maxChainIdLength: SIGNET_CONFIG.maxChainIdLength,
        maxEvmDataLength: SIGNET_CONFIG.maxEvmDataLength,
        chainId: chainIdHex,
      },
    },
    EthDispenser: {
      DispenserConfig: {
        paused: false,
        faucetBalanceWei: DISPENSER_CONFIG.faucetBalanceWei.toString(),
        faucetAddress: DISPENSER_CONFIG.faucetAddress,
        minFaucetThreshold: DISPENSER_CONFIG.minFaucetThreshold.toString(),
        minRequest: DISPENSER_CONFIG.minRequest.toString(),
        maxDispense: DISPENSER_CONFIG.maxDispense.toString(),
        dispenserFee: DISPENSER_CONFIG.dispenserFee.toString(),
      },
    },
  })

  await (api.rpc as any)('dev_newBlock', { count: 1 })
  console.log('Storage set in new block.')
}

// ---- Real network: TC proposal ----

async function proposeViaTechCommittee(api: ApiPromise) {
  const suri = process.env.SURI
  if (!suri) {
    console.error('Error: SURI env var required for real networks (e.g. SURI=//Alice or SURI="mnemonic words...")')
    process.exit(1)
  }

  const keyring = new Keyring({ type: 'sr25519' })
  const signer = keyring.addFromUri(suri)
  console.log(`Signer (TC member): ${signer.address}`)

  const { signetCall, dispenserCall } = buildCalls(api)

  // Batch both setConfig calls
  const batchCall = api.tx.utility.batchAll([signetCall, dispenserCall])

  // Get TC member count for threshold (majority = floor(n/2) + 1)
  const members = await (api.query as any).technicalCommittee.members()
  const memberCount = (members.toJSON() as any[]).length
  const threshold = Math.floor(memberCount / 2) + 1

  console.log(`TC members: ${memberCount}, threshold: ${threshold}`)

  // Propose via TC
  const lengthBound = batchCall.method.encodedLength + 100
  const proposeTx = (api.tx as any).technicalCommittee.propose(
    threshold,
    batchCall,
    lengthBound,
  )

  console.log('Submitting TC proposal...')
  await new Promise<void>((resolve, reject) => {
    proposeTx.signAndSend(signer, { nonce: -1 }, (result: any) => {
      if (result.dispatchError) {
        if (result.dispatchError.isModule) {
          const decoded = api.registry.findMetaError(result.dispatchError.asModule)
          reject(new Error(`${decoded.section}.${decoded.name}: ${decoded.docs.join(' ')}`))
        } else {
          reject(new Error(result.dispatchError.toString()))
        }
      } else if (result.status.isInBlock) {
        console.log(`Proposal included in block: ${result.status.asInBlock.toHex()}`)

        // Extract proposal index from events
        for (const { event } of result.events) {
          if (event.section === 'technicalCommittee' && event.method === 'Proposed') {
            const [, , proposalIndex] = event.data
            console.log(`Proposal index: ${proposalIndex.toString()}`)
            console.log(`Other TC members need to vote Aye on proposal #${proposalIndex}`)
          }
        }
        resolve()
      }
    }).catch(reject)
  })
}

// ---- Verify ----

async function verifyConfigs(api: ApiPromise) {
  console.log('\n--- Verifying configs ---')

  const signetCfg = await (api.query as any).signet.signetConfig()
  console.log('Signet config:', signetCfg.toJSON())

  const dispenserCfg = await (api.query as any).ethDispenser.dispenserConfig()
  console.log('Dispenser config:', dispenserCfg.toJSON())
}

// ---- Main ----

async function main() {
  console.log(`Network: ${networkName}`)
  console.log(`Connecting to ${wsEndpoint}...`)
  console.log(`Signet chain ID: ${chainId}`)

  const provider = new WsProvider(wsEndpoint, undefined, undefined, 180_000)
  const api = await ApiPromise.create({ provider })
  console.log(`Connected to chain: ${(await api.rpc.system.chain()).toString()}`)

  const chopsticks = await isChopsticks(api)

  if (chopsticks) {
    console.log('Mode: Chopsticks (dev_setStorage)\n')
    await executeOnChopsticks(api)
  } else {
    console.log('Mode: Real network (TC proposal)\n')
    await proposeViaTechCommittee(api)
  }

  await verifyConfigs(api)
  await api.disconnect()
  console.log('\nDone.')
}

main().catch((err) => {
  console.error('Error:', err)
  process.exit(1)
})
