export const FEE_TICK_SPACING: Readonly<Record<number, number>> = {
  100: 1,
  500: 10,
  3000: 60,
  10000: 200,
}

export const MIN_TICK = -887272
export const MAX_TICK = 887272

export function isqrt(n: bigint): bigint {
  if (n < 0n) throw new Error('isqrt of negative')
  if (n < 2n) return n
  let x = n
  let y = (x + 1n) / 2n
  while (y < x) {
    x = y
    y = (x + n / x) / 2n
  }
  return x
}

export function sqrtPriceX96FromAmounts(amount0: bigint, amount1: bigint): bigint {
  if (amount0 <= 0n) throw new Error('amount0 must be positive')
  return isqrt((amount1 << 192n) / amount0)
}

export function fullRangeTicks(fee: number): [number, number] {
  const spacing = FEE_TICK_SPACING[fee]
  if (spacing === undefined) throw new Error(`unknown fee tier: ${fee}`)
  return [Math.ceil(MIN_TICK / spacing) * spacing, Math.floor(MAX_TICK / spacing) * spacing]
}
