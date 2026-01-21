declare module 'coinselect' {
  export interface UTXO {
    txid: string;
    vout: number;
    value: number;
    script?: Buffer;
  }

  export interface Target {
    address?: string;
    script?: Buffer;
    value: number;
  }

  export interface CoinSelectResult {
    inputs: UTXO[] | undefined;
    outputs: Array<{ address?: string; script?: Buffer; value: number }> | undefined;
    fee: number;
  }

  function coinSelect(
    utxos: UTXO[],
    targets: Target[],
    feeRate: number
  ): CoinSelectResult;

  export default coinSelect;
}