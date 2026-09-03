import { Contract, ZeroAddress } from 'ethers'
import { config, requireDeployed } from '../config'
import { ERC20_ABI, FACTORY_ABI, NPM_ABI, POOL_ABI } from '../core/abis'
import { assetToEvmAddress, sortTokens, truncatedAccountId } from '../core/assets'
import { SequentialTxRunner, evmProvider, evmWallet } from '../core/evm'
import { FEE_TICK_SPACING, sqrtPriceX96FromAmounts } from '../core/math'

const CONC_FEE = 500

const POSITIONS = [
  { tickLower: -200, tickUpper: 200, amount0: 3n * 10n ** 23n, amount1: 3n * 10n ** 23n },
  { tickLower: -2000, tickUpper: 2000, amount0: 1n * 10n ** 23n, amount1: 1n * 10n ** 23n },
]

const POOL_TICKS_ABI = [
  ...POOL_ABI,
  'function ticks(int24 tick) view returns (uint128 liquidityGross, int128 liquidityNet, uint256 feeGrowthOutside0X128, uint256 feeGrowthOutside1X128, int56 tickCumulativeOutside, uint160 secondsPerLiquidityOutsideX128, uint32 secondsOutside, bool initialized)',
]

export async function seedConcentrated(): Promise<void> {
  const { factory: factoryAddress, npm: npmAddress } = requireDeployed(config)
  const spacing = FEE_TICK_SPACING[CONC_FEE]
  if (spacing === undefined) throw new Error(`unknown fee tier: ${CONC_FEE}`)
  for (const p of POSITIONS) {
    if (p.tickLower % spacing !== 0 || p.tickUpper % spacing !== 0) {
      throw new Error(`tick not aligned to spacing ${spacing}: [${p.tickLower}, ${p.tickUpper}]`)
    }
  }

  const provider = evmProvider(config.rpcEvm)
  const wallet = evmWallet(config.evmPrivateKey, provider)

  const addrA = assetToEvmAddress(config.tokenA)
  const addrB = assetToEvmAddress(config.tokenB)
  const [token0, token1] = sortTokens(addrA, addrB)

  const total0 = POSITIONS.reduce((a, p) => a + p.amount0, 0n)
  const total1 = POSITIONS.reduce((a, p) => a + p.amount1, 0n)

  console.log(`deployer ${wallet.address} (mapped ${truncatedAccountId(wallet.address)})`)
  console.log(`pair token0 ${token0} token1 ${token1} | fee ${CONC_FEE} spacing ${spacing}`)
  console.log(`positions ${POSITIONS.map((p) => `[${p.tickLower},${p.tickUpper}]`).join(' ')}`)

  const erc0 = new Contract(token0, ERC20_ABI, wallet)
  const erc1 = new Contract(token1, ERC20_ABI, wallet)
  const [bal0, bal1, gas] = await Promise.all([
    erc0.balanceOf(wallet.address) as Promise<bigint>,
    erc1.balanceOf(wallet.address) as Promise<bigint>,
    provider.getBalance(wallet.address),
  ])
  console.log(`balance0 ${bal0} balance1 ${bal1} gas ${gas}`)
  if (bal0 < total0 || bal1 < total1 || gas === 0n) {
    throw new Error('insufficient deployer balance/gas — run `fund` first')
  }

  const runner = new SequentialTxRunner(wallet, config.evmGasLimit)
  await runner.init()

  const factory = new Contract(factoryAddress, FACTORY_ABI, wallet)
  let pool: string = await factory.getPool(token0, token1, CONC_FEE)
  if (pool === ZeroAddress) {
    console.log('creating pool...')
    await runner.confirm('createPool', factory.createPool(token0, token1, CONC_FEE, runner.next()))
    pool = await factory.getPool(token0, token1, CONC_FEE)
  }
  if (pool === ZeroAddress) throw new Error('pool creation failed')
  console.log(`pool ${pool}`)

  const poolContract = new Contract(pool, POOL_TICKS_ABI, wallet)
  let sqrtNow = 0n
  try {
    sqrtNow = (await poolContract.slot0())[0] as bigint
  } catch {
    sqrtNow = 0n
  }
  if (sqrtNow === 0n) {
    const sqrtPrice = sqrtPriceX96FromAmounts(1n, 1n)
    console.log(`initialize sqrtPriceX96 ${sqrtPrice}`)
    await runner.confirm('initialize', poolContract.initialize(sqrtPrice, runner.next()))
  } else {
    console.log(`already initialized sqrtPriceX96 ${sqrtNow}`)
  }

  console.log('approving NPM...')
  await runner.confirm('approve0', erc0.approve(npmAddress, total0, runner.next()))
  await runner.confirm('approve1', erc1.approve(npmAddress, total1, runner.next()))

  const npm = new Contract(npmAddress, NPM_ABI, wallet)
  const deadline = BigInt(Math.floor(Date.now() / 1000) + 3600)
  for (const p of POSITIONS) {
    const params = [
      token0,
      token1,
      CONC_FEE,
      p.tickLower,
      p.tickUpper,
      p.amount0,
      p.amount1,
      0n,
      0n,
      wallet.address,
      deadline,
    ]
    console.log(`mint [${p.tickLower}, ${p.tickUpper}]...`)
    await runner.confirm(`mint[${p.tickLower},${p.tickUpper}]`, npm.mint(params, runner.next()))
  }

  const [slot0, liquidity] = await Promise.all([
    poolContract.slot0(),
    poolContract.liquidity() as Promise<bigint>,
  ])
  console.log('\n=== seeded concentrated pool ===')
  console.log(`pool ${pool} fee ${CONC_FEE}`)
  console.log(`sqrtPriceX96 ${slot0[0]} tick ${slot0[1]}`)
  console.log(`in-range liquidity ${liquidity}`)
  for (const t of [-2000, -200, 200, 2000]) {
    const info = await poolContract.ticks(t)
    console.log(`tick ${t}: liquidityNet ${info[1]} initialized ${info[7]}`)
  }
}
