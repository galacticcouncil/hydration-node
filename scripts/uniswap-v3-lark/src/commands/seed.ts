import { Contract, ZeroAddress } from 'ethers'
import { config, requireDeployed } from '../config'
import { ERC20_ABI, FACTORY_ABI, NPM_ABI, POOL_ABI } from '../core/abis'
import { assetToEvmAddress, sortTokens, truncatedAccountId } from '../core/assets'
import { SequentialTxRunner, evmProvider, evmWallet } from '../core/evm'
import { FEE_TICK_SPACING, fullRangeTicks, sqrtPriceX96FromAmounts } from '../core/math'

export async function seed(): Promise<void> {
  const { factory: factoryAddress, npm: npmAddress } = requireDeployed(config)

  const provider = evmProvider(config.rpcEvm)
  const wallet = evmWallet(config.evmPrivateKey, provider)

  const addrA = assetToEvmAddress(config.tokenA)
  const addrB = assetToEvmAddress(config.tokenB)
  const [token0, token1] = sortTokens(addrA, addrB)
  const isA0 = token0 === addrA
  const amount0 = isA0 ? config.amountA : config.amountB
  const amount1 = isA0 ? config.amountB : config.amountA

  console.log(`deployer ${wallet.address} (mapped ${truncatedAccountId(wallet.address)})`)
  console.log(`token0 ${token0} amount0 ${amount0}`)
  console.log(`token1 ${token1} amount1 ${amount1}`)
  console.log(`fee ${config.fee} spacing ${FEE_TICK_SPACING[config.fee]}`)

  const erc0 = new Contract(token0, ERC20_ABI, wallet)
  const erc1 = new Contract(token1, ERC20_ABI, wallet)
  const [bal0, bal1, gas] = await Promise.all([
    erc0.balanceOf(wallet.address) as Promise<bigint>,
    erc1.balanceOf(wallet.address) as Promise<bigint>,
    provider.getBalance(wallet.address),
  ])
  console.log(`balance0 ${bal0} balance1 ${bal1} gas ${gas}`)
  if (bal0 < amount0 || bal1 < amount1 || gas === 0n) {
    throw new Error('insufficient deployer balance/gas — run `fund` first')
  }

  const runner = new SequentialTxRunner(wallet, config.evmGasLimit)
  await runner.init()

  const factory = new Contract(factoryAddress, FACTORY_ABI, wallet)
  let pool: string = await factory.getPool(token0, token1, config.fee)
  if (pool === ZeroAddress) {
    console.log('creating pool...')
    await runner.confirm('createPool', factory.createPool(token0, token1, config.fee, runner.next()))
    pool = await factory.getPool(token0, token1, config.fee)
  }
  if (pool === ZeroAddress) throw new Error('pool creation failed')
  console.log(`pool ${pool}`)

  const poolContract = new Contract(pool, POOL_ABI, wallet)
  let sqrtNow = 0n
  try {
    sqrtNow = (await poolContract.slot0())[0] as bigint
  } catch {
    sqrtNow = 0n
  }
  if (sqrtNow === 0n) {
    const sqrtPrice = sqrtPriceX96FromAmounts(amount0, amount1)
    console.log(`initialize sqrtPriceX96 ${sqrtPrice}`)
    await runner.confirm('initialize', poolContract.initialize(sqrtPrice, runner.next()))
  } else {
    console.log(`already initialized sqrtPriceX96 ${sqrtNow}`)
  }

  console.log('approving NPM...')
  await runner.confirm('approve0', erc0.approve(npmAddress, amount0, runner.next()))
  await runner.confirm('approve1', erc1.approve(npmAddress, amount1, runner.next()))

  const [tickLower, tickUpper] = fullRangeTicks(config.fee)
  const deadline = BigInt(Math.floor(Date.now() / 1000) + 3600)
  const params = [token0, token1, config.fee, tickLower, tickUpper, amount0, amount1, 0n, 0n, wallet.address, deadline]
  const npm = new Contract(npmAddress, NPM_ABI, wallet)
  console.log(`mint full-range [${tickLower}, ${tickUpper}]...`)
  await runner.confirm('mint', npm.mint(params, runner.next()))

  const liquidity = (await poolContract.liquidity()) as bigint
  console.log(`pool ${pool} liquidity ${liquidity}`)
}
