import { config as loadEnv } from 'dotenv'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { z } from 'zod'

export const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
loadEnv({ path: resolve(packageRoot, '.env') })

const ANVIL_KEY = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80'

const Raw = z.object({
  RPC_WS: z.string().url().default('wss://1.lark.hydration.cloud'),
  RPC_EVM: z.string().url().default('https://1.lark.hydration.cloud'),
  SURI: z.string().default('//Alice'),
  EVM_PRIVATE_KEY: z
    .string()
    .regex(/^0x[0-9a-fA-F]{64}$/, 'EVM_PRIVATE_KEY must be 0x + 64 hex')
    .default(ANVIL_KEY),

  UNISWAP_V3_FACTORY: z.string().default(''),
  UNISWAP_V3_SWAP_ROUTER: z.string().default(''),
  UNISWAP_V3_QUOTER: z.string().default(''),
  UNISWAP_V3_NPM: z.string().default(''),
  UNISWAP_DEPLOY_CLI: z.string().default(''),

  TOKEN_A: z.coerce.number().int().default(16),
  TOKEN_B: z.coerce.number().int().default(9),
  FEE: z.coerce.number().int().default(3000),
  AMOUNT_A: z.string().default('1000000000000000000000000'),
  AMOUNT_B: z.string().default('1000000000000000000000000'),

  GAS_ASSET_ID: z.coerce.number().int().default(20),
  FUND_GAS_AMOUNT: z.string().default('100000000000000000000'),
  FUND_AMOUNT_A: z.string().default('2000000000000000000000000'),
  FUND_AMOUNT_B: z.string().default('2000000000000000000000000'),

  SELL_ASSET_IN: z.coerce.number().int().default(0),
  QUOTE_ASSET: z.coerce.number().int().default(9),
  SELL_AMOUNT: z.string().default('1000000000000000'),
  FEES: z.string().default('500,3000,10000'),

  VOTE_HDX: z.coerce.number().int().default(3_000_000_000),
  ENACT_AFTER: z.coerce.number().int().default(10),
  EVM_GAS_LIMIT: z.string().default('15000000'),
})

const raw = Raw.parse(process.env)
const optional = (s: string): string | undefined => (s.trim() === '' ? undefined : s.trim())

export const config = {
  rpcWs: raw.RPC_WS,
  rpcEvm: raw.RPC_EVM,
  suri: raw.SURI,
  evmPrivateKey: raw.EVM_PRIVATE_KEY,
  factory: optional(raw.UNISWAP_V3_FACTORY),
  swapRouter: optional(raw.UNISWAP_V3_SWAP_ROUTER),
  quoter: optional(raw.UNISWAP_V3_QUOTER),
  npm: optional(raw.UNISWAP_V3_NPM),
  deployCli: optional(raw.UNISWAP_DEPLOY_CLI) ?? resolve(packageRoot, '../../../uniswap-v3-deploy/dist/index.js'),
  tokenA: raw.TOKEN_A,
  tokenB: raw.TOKEN_B,
  fee: raw.FEE,
  amountA: BigInt(raw.AMOUNT_A),
  amountB: BigInt(raw.AMOUNT_B),
  gasAssetId: raw.GAS_ASSET_ID,
  fundGasAmount: BigInt(raw.FUND_GAS_AMOUNT),
  fundAmountA: BigInt(raw.FUND_AMOUNT_A),
  fundAmountB: BigInt(raw.FUND_AMOUNT_B),
  sellAssetIn: raw.SELL_ASSET_IN,
  quoteAsset: raw.QUOTE_ASSET,
  sellAmount: BigInt(raw.SELL_AMOUNT),
  fees: raw.FEES.split(',').map((s) => Number(s.trim())),
  voteHdx: BigInt(raw.VOTE_HDX),
  enactAfter: raw.ENACT_AFTER,
  evmGasLimit: BigInt(raw.EVM_GAS_LIMIT),
} as const

export type Config = typeof config

export interface DeployedAddresses {
  factory: string
  swapRouter: string
  quoter: string
  npm: string
}

export function requireDeployed(c: Config): DeployedAddresses {
  const missing = (['factory', 'swapRouter', 'quoter', 'npm'] as const).filter((k) => !c[k])
  if (missing.length > 0) {
    throw new Error(`missing deployed addresses in .env: ${missing.join(', ')} — run \`deploy\` first`)
  }
  return { factory: c.factory!, swapRouter: c.swapRouter!, quoter: c.quoter!, npm: c.npm! }
}
