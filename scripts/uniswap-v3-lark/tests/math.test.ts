import { describe, expect, it } from 'vitest'
import { fullRangeTicks, isqrt, sqrtPriceX96FromAmounts } from '../src/core/math'

describe('isqrt', () => {
  it('floors the integer square root', () => {
    expect(isqrt(0n)).toBe(0n)
    expect(isqrt(1n)).toBe(1n)
    expect(isqrt(4n)).toBe(2n)
    expect(isqrt(8n)).toBe(2n)
    expect(isqrt(9n)).toBe(3n)
    expect(isqrt(1n << 192n)).toBe(1n << 96n)
  })

  it('throws on negative input', () => {
    expect(() => isqrt(-1n)).toThrow()
  })
})

describe('sqrtPriceX96FromAmounts', () => {
  it('returns 2^96 for equal amounts (price 1)', () => {
    expect(sqrtPriceX96FromAmounts(10n ** 24n, 10n ** 24n)).toBe(2n ** 96n)
  })

  it('returns 2^97 for 4:1 amounts (price 4)', () => {
    expect(sqrtPriceX96FromAmounts(1n, 4n)).toBe(2n ** 97n)
  })

  it('throws on zero amount0', () => {
    expect(() => sqrtPriceX96FromAmounts(0n, 1n)).toThrow()
  })
})

describe('fullRangeTicks', () => {
  it('aligns the full range to the fee tier spacing', () => {
    expect(fullRangeTicks(3000)).toEqual([-887220, 887220])
    expect(fullRangeTicks(500)).toEqual([-887270, 887270])
    expect(fullRangeTicks(10000)).toEqual([-887200, 887200])
    expect(fullRangeTicks(100)).toEqual([-887272, 887272])
  })

  it('throws on an unknown fee tier', () => {
    expect(() => fullRangeTicks(1234)).toThrow()
  })
})
