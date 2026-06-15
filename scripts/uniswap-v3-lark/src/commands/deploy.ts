import { spawn } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { Wallet } from 'ethers'
import { config, packageRoot } from '../config'
import { assetToEvmAddress } from '../core/assets'

export async function deploy(): Promise<void> {
  if (!existsSync(config.deployCli)) {
    throw new Error(`uniswap deploy CLI not found at ${config.deployCli} — set UNISWAP_DEPLOY_CLI`)
  }

  const owner = new Wallet(config.evmPrivateKey).address
  const statePath = resolve(packageRoot, 'state.json')
  const weth9 = assetToEvmAddress(config.gasAssetId)

  const args = [
    config.deployCli,
    '--private-key', config.evmPrivateKey,
    '--json-rpc', config.rpcEvm,
    '--weth9-address', weth9,
    '--native-currency-label', 'WETH',
    '--owner-address', owner,
    '--state', statePath,
    '--confirmations', '3',
  ]

  console.log(`deploying uniswap v3 stack via ${config.deployCli}`)
  console.log(`state -> ${statePath}`)

  await new Promise<void>((resolvePromise, reject) => {
    const child = spawn('node', args, { stdio: 'inherit' })
    child.on('error', reject)
    child.on('exit', (code) => (code === 0 ? resolvePromise() : reject(new Error(`deploy exited with code ${code}`))))
  })

  const state = JSON.parse(readFileSync(statePath, 'utf8')) as Record<string, string>
  console.log('\ndeploy complete — set these in .env:')
  console.log(`UNISWAP_V3_FACTORY=${state['v3CoreFactoryAddress']}`)
  console.log(`UNISWAP_V3_SWAP_ROUTER=${state['swapRouter02']}`)
  console.log(`UNISWAP_V3_QUOTER=${state['quoterV2Address']}`)
  console.log(`UNISWAP_V3_NPM=${state['nonfungibleTokenPositionManagerAddress']}`)
}
