import { config } from 'dotenv'
import { ethers } from 'ethers'
import path from 'path'
import {
  SUBSTRATE_PRESETS,
  EVM_PRESETS,
  DEFAULT_ROOT_PUBLIC_KEY,
  DEFAULT_FAUCET_ADDRESS,
  DEFAULT_TARGET_ADDRESS,
  DEFAULT_REQUEST_FUND_AMOUNT_WEI,
  type SubstrateNetwork,
  type EvmNetwork,
} from './networks'

config({ path: path.resolve(__dirname, '.env') })

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function env(key: string): string | undefined {
  return process.env[key]
}

function envRequired(key: string): string {
  const value = env(key)
  if (!value) throw new Error(`Missing required env variable: ${key}`)
  return value
}

function envInt(key: string, fallback: number): number {
  const value = env(key)
  if (!value) return fallback
  const parsed = parseInt(value, 10)
  if (isNaN(parsed)) throw new Error(`${key} must be an integer, got: ${value}`)
  return parsed
}

function envBigInt(key: string, fallback: bigint): bigint {
  const value = env(key)
  if (!value) return fallback
  try {
    return BigInt(value)
  } catch {
    throw new Error(`${key} must be a bigint, got: ${value}`)
  }
}

// ---------------------------------------------------------------------------
// Load network presets
// ---------------------------------------------------------------------------

const validSubstrate = Object.keys(SUBSTRATE_PRESETS)
const validEvm = Object.keys(EVM_PRESETS)

const SUBSTRATE_NETWORK = envRequired('SUBSTRATE_NETWORK') as SubstrateNetwork
if (!validSubstrate.includes(SUBSTRATE_NETWORK)) {
  throw new Error(
    `SUBSTRATE_NETWORK must be one of: ${validSubstrate.join(', ')}. Got: ${SUBSTRATE_NETWORK}`,
  )
}

const EVM_NETWORK = envRequired('EVM_NETWORK') as EvmNetwork
if (!validEvm.includes(EVM_NETWORK)) {
  throw new Error(
    `EVM_NETWORK must be one of: ${validEvm.join(', ')}. Got: ${EVM_NETWORK}`,
  )
}

const substrate = SUBSTRATE_PRESETS[SUBSTRATE_NETWORK]
const evm = EVM_PRESETS[EVM_NETWORK]

// ---------------------------------------------------------------------------
// Resolve final values (env overrides take precedence over presets)
// ---------------------------------------------------------------------------

const SUBSTRATE_WS_ENDPOINT = env('SUBSTRATE_WS_ENDPOINT') || substrate.wsEndpoint
const SUBSTRATE_CHAIN_ID = env('SUBSTRATE_CHAIN_ID') || substrate.chainId
const SS58_PREFIX = envInt('SS58_PREFIX', substrate.ss58Prefix)

const EVM_RPC_URL = env('EVM_RPC_URL') || evm.rpcUrl
const EVM_CHAIN_ID = envInt('EVM_CHAIN_ID', evm.chainId)

const ROOT_PUBLIC_KEY = env('ROOT_PUBLIC_KEY') || DEFAULT_ROOT_PUBLIC_KEY
const FAUCET_ADDRESS = env('FAUCET_ADDRESS') || DEFAULT_FAUCET_ADDRESS
const TARGET_ADDRESS = env('TARGET_ADDRESS') || DEFAULT_TARGET_ADDRESS
const REQUEST_FUND_AMOUNT = envBigInt('REQUEST_FUND_AMOUNT_WEI', DEFAULT_REQUEST_FUND_AMOUNT_WEI)

const GAS_LIMIT = envBigInt('GAS_LIMIT', 100_000n)
const DEFAULT_MAX_FEE_PER_GAS = envBigInt('DEFAULT_MAX_FEE_PER_GAS', 30_000_000_000n)
const DEFAULT_MAX_PRIORITY_FEE_PER_GAS = envBigInt('DEFAULT_MAX_PRIORITY_FEE_PER_GAS', 2_000_000_000n)

// Validate critical values
if (!ethers.isAddress(FAUCET_ADDRESS)) throw new Error(`Invalid FAUCET_ADDRESS: ${FAUCET_ADDRESS}`)
if (!ethers.isAddress(TARGET_ADDRESS)) throw new Error(`Invalid TARGET_ADDRESS: ${TARGET_ADDRESS}`)
if (!/^0x[0-9a-fA-F]+$/.test(ROOT_PUBLIC_KEY)) throw new Error(`Invalid ROOT_PUBLIC_KEY`)

export const ENV = {
  SUBSTRATE_NETWORK,
  EVM_NETWORK,

  // Substrate
  SUBSTRATE_WS_ENDPOINT,
  SUBSTRATE_CHAIN_ID,
  SS58_PREFIX,

  // EVM
  EVM_RPC_URL,
  EVM_CHAIN_ID,
  ROOT_PUBLIC_KEY,
  FAUCET_ADDRESS,

  // Test params
  TARGET_ADDRESS,
  REQUEST_FUND_AMOUNT,

  // Gas
  GAS_LIMIT,
  DEFAULT_MAX_FEE_PER_GAS,
  DEFAULT_MAX_PRIORITY_FEE_PER_GAS,
} as const

// Print resolved config on load
console.log(`\n--- Dispenser Test Config ---`)
console.log(`  Substrate:        ${ENV.SUBSTRATE_NETWORK} (${ENV.SUBSTRATE_WS_ENDPOINT})`)
console.log(`  EVM:              ${ENV.EVM_NETWORK} (${ENV.EVM_RPC_URL})`)
console.log(`  Chain ID (CAIP2): ${ENV.SUBSTRATE_CHAIN_ID}`)
console.log(`  EVM Chain ID:     ${ENV.EVM_CHAIN_ID}`)
console.log(`  Faucet contract:  ${ENV.FAUCET_ADDRESS}`)
console.log(`  Target address:   ${ENV.TARGET_ADDRESS}`)
console.log(`  Request amount:   ${ENV.REQUEST_FUND_AMOUNT} wei`)
console.log(`----------------------------\n`)
