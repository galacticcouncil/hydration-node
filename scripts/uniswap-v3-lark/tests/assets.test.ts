import { describe, expect, it } from 'vitest'
import { assetToEvmAddress, sortTokens, truncatedAccountId } from '../src/core/assets'

describe('assetToEvmAddress', () => {
  it('maps an asset id to its erc20-precompile address', () => {
    expect(assetToEvmAddress(0)).toBe('0x0000000000000000000000000000000100000000')
    expect(assetToEvmAddress(2)).toBe('0x0000000000000000000000000000000100000002')
    expect(assetToEvmAddress(20)).toBe('0x0000000000000000000000000000000100000014')
  })

  it('throws on a negative id', () => {
    expect(() => assetToEvmAddress(-1)).toThrow()
  })
})

describe('truncatedAccountId', () => {
  it('prefixes ETH\\0 and right-pads the address with zeros', () => {
    expect(truncatedAccountId('0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266')).toBe(
      '0x45544800f39fd6e51aad88f6f4ce6ab8827279cfffb922660000000000000000',
    )
  })
})

describe('sortTokens', () => {
  it('orders a pair ascending by address', () => {
    const lo = '0x0000000000000000000000000000000100000009'
    const hi = '0x0000000000000000000000000000000100000010'
    expect(sortTokens(hi, lo)).toEqual([lo, hi])
    expect(sortTokens(lo, hi)).toEqual([lo, hi])
  })
})
