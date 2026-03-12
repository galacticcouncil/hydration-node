// ---------------------------------------------------------------------------
// Network presets — define SUBSTRATE_NETWORK + EVM_NETWORK in .env and
// everything else is derived automatically. Any value can be overridden
// via env vars.
// ---------------------------------------------------------------------------

export type SubstrateNetwork = 'chopsticks' | 'lark' | 'mainnet'
export type EvmNetwork = 'anvil' | 'sepolia' | 'mainnet'

export interface SubstratePreset {
  wsEndpoint: string
  /** CAIP-2 chain ID used for MPC key derivation (must match signet on-chain config) */
  chainId: string
  ss58Prefix: number
}

export interface EvmPreset {
  rpcUrl: string
  chainId: number
}

// ---- Substrate presets ----

export const SUBSTRATE_PRESETS: Record<SubstrateNetwork, SubstratePreset> = {
  chopsticks: {
    wsEndpoint: 'ws://localhost:8000',
    // Chopsticks forks mainnet but tc-set-config.ts writes the lark chain ID
    // by default. Override with SUBSTRATE_CHAIN_ID if needed.
    chainId: 'polkadot:e6b50b06e72a81194e9c96c488175ecd',
    ss58Prefix: 63,
  },
  lark: {
    wsEndpoint: 'wss://1.lark.hydration.cloud',
    chainId: 'polkadot:e6b50b06e72a81194e9c96c488175ecd',
    ss58Prefix: 63,
  },
  mainnet: {
    wsEndpoint: 'wss://rpc.hydradx.cloud',
    chainId: 'polkadot:afdc188f45c71dacbaa0b62e16a91f72',
    ss58Prefix: 63,
  },
}

// ---- EVM presets ----

export const EVM_PRESETS: Record<EvmNetwork, EvmPreset> = {
  anvil: {
    rpcUrl: 'http://localhost:8545',
    chainId: 31337,
  },
  sepolia: {
    rpcUrl: 'https://ethereum-sepolia-rpc.publicnode.com',
    chainId: 11155111,
  },
  mainnet: {
    rpcUrl: 'https://eth.llamarpc.com',
    chainId: 1,
  },
}

// ---- Shared defaults ----

/** MPC root public key (uncompressed secp256k1, same across all networks) */
export const DEFAULT_ROOT_PUBLIC_KEY =
  '0x048318535b54105d4a7aae60c08fc45f9687181b4fdfc625bd1a753fa7397fed753547f11ca8696646f2f3acb08e31016afac23e630c5d11f59f61fef57b0d2aa5'

/** GasFaucet contract address (current deployment) */
export const DEFAULT_FAUCET_ADDRESS = '0x189d33ea9A9701fdb67C21df7420868193dcf578'

/** Default test target address */
export const DEFAULT_TARGET_ADDRESS = '0x7f67681ce8c292bbbef0ccfa1475d9742b6ab3ac'

/** Default request amount — must be above WETH existential deposit (~5.4e12) */
export const DEFAULT_REQUEST_FUND_AMOUNT_WEI = 100_000_000_000_000n // 0.0001 ETH
