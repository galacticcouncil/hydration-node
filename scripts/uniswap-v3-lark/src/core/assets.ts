import { getAddress } from 'ethers'

export function assetToEvmAddress(assetId: number): string {
  if (!Number.isInteger(assetId) || assetId < 0) {
    throw new Error(`invalid asset id: ${assetId}`)
  }
  const value = 0x0100000000n + BigInt(assetId)
  return getAddress('0x' + value.toString(16).padStart(40, '0'))
}

export function truncatedAccountId(evmAddress: string): string {
  const body = evmAddress.toLowerCase().replace(/^0x/, '').padStart(40, '0')
  return '0x45544800' + body + '0000000000000000'
}

export function sortTokens(a: string, b: string): [string, string] {
  return a.toLowerCase() < b.toLowerCase() ? [a, b] : [b, a]
}
