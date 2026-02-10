declare module 'coinselect' {
  interface UTXO {
    txid: string
    vout: number
    value: number
    script: Buffer
    [key: string]: any
  }

  interface Target {
    script: Buffer
    value: number
  }

  interface CoinSelectResult {
    inputs?: UTXO[]
    outputs?: (Target & { [key: string]: any })[]
    fee: number
  }

  export default function coinSelect(
    utxos: UTXO[],
    targets: Target[],
    feeRate: number,
  ): CoinSelectResult
}
