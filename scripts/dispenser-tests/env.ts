import { config } from 'dotenv'
import { ethers } from 'ethers'
import path from 'path'

config({ path: path.resolve(__dirname, '.env') })

function required(key: string): string {
  const value = process.env[key]
  if (!value) {
    throw new Error(`Missing required env variable: ${key}`)
  }
  return value
}

function optionalInt(key: string, fallback: number): number {
  const value = process.env[key]
  if (!value) return fallback
  const parsed = parseInt(value, 10)
  if (isNaN(parsed)) {
    throw new Error(`Env variable ${key} must be an integer, got: ${value}`)
  }
  return parsed
}

function optionalBigInt(key: string, fallback: bigint): bigint {
  const value = process.env[key]
  if (!value) return fallback
  try {
    return BigInt(value)
  } catch {
    throw new Error(`Env variable ${key} must be a bigint, got: ${value}`)
  }
}

function validateEthAddress(key: string, value: string): string {
  if (!ethers.isAddress(value)) {
    throw new Error(`Env variable ${key} is not a valid Ethereum address: ${value}`)
  }
  return value
}

function validateUrl(key: string, value: string): string {
  try {
    new URL(value)
  } catch {
    throw new Error(`Env variable ${key} is not a valid URL: ${value}`)
  }
  return value
}

function validateHexKey(key: string, value: string): string {
  if (!/^0x[0-9a-fA-F]+$/.test(value)) {
    throw new Error(`Env variable ${key} is not valid hex: ${value}`)
  }
  return value
}

// --- Load and validate ---

const NETWORK = required('NETWORK') // 'local' | 'sepolia' | 'mainnet'
const validNetworks = ['local', 'sepolia', 'mainnet'] as const
if (!validNetworks.includes(NETWORK as any)) {
  throw new Error(
    `NETWORK must be one of: ${validNetworks.join(', ')}. Got: ${NETWORK}`,
  )
}

const EVM_RPC_URL = validateUrl('EVM_RPC_URL', required('EVM_RPC_URL'))
const EVM_CHAIN_ID = optionalInt('EVM_CHAIN_ID', NETWORK === 'sepolia' ? 11155111 : NETWORK === 'mainnet' ? 1 : 31337)
const ROOT_PUBLIC_KEY = validateHexKey('ROOT_PUBLIC_KEY', required('ROOT_PUBLIC_KEY'))
const FAUCET_ADDRESS = validateEthAddress('FAUCET_ADDRESS', required('FAUCET_ADDRESS'))

const SUBSTRATE_WS_ENDPOINT = validateUrl('SUBSTRATE_WS_ENDPOINT', required('SUBSTRATE_WS_ENDPOINT'))
const SUBSTRATE_CHAIN_ID = required('SUBSTRATE_CHAIN_ID') // e.g. 'polkadot:2034'
const SS58_PREFIX = optionalInt('SS58_PREFIX', 0)

const TARGET_ADDRESS = validateEthAddress('TARGET_ADDRESS', required('TARGET_ADDRESS'))
const REQUEST_FUND_AMOUNT = optionalBigInt('REQUEST_FUND_AMOUNT_WEI', 1_000_000_000_000n) // 0.000001 ETH

const GAS_LIMIT = optionalBigInt('GAS_LIMIT', 100_000n)
const DEFAULT_MAX_FEE_PER_GAS = optionalBigInt('DEFAULT_MAX_FEE_PER_GAS', 30_000_000_000n)
const DEFAULT_MAX_PRIORITY_FEE_PER_GAS = optionalBigInt('DEFAULT_MAX_PRIORITY_FEE_PER_GAS', 2_000_000_000n)

export const ENV = {
  NETWORK: NETWORK as 'local' | 'sepolia' | 'mainnet',

  // EVM
  EVM_RPC_URL,
  EVM_CHAIN_ID,
  ROOT_PUBLIC_KEY,
  FAUCET_ADDRESS,

  // Substrate
  SUBSTRATE_WS_ENDPOINT,
  SUBSTRATE_CHAIN_ID,
  SS58_PREFIX,

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
console.log(`  Network:          ${ENV.NETWORK}`)
console.log(`  EVM RPC:          ${ENV.EVM_RPC_URL}`)
console.log(`  EVM Chain ID:     ${ENV.EVM_CHAIN_ID}`)
console.log(`  Faucet contract:  ${ENV.FAUCET_ADDRESS}`)
console.log(`  Substrate WS:     ${ENV.SUBSTRATE_WS_ENDPOINT}`)
console.log(`  Substrate Chain:  ${ENV.SUBSTRATE_CHAIN_ID}`)
console.log(`  Target address:   ${ENV.TARGET_ADDRESS}`)
console.log(`----------------------------\n`)
