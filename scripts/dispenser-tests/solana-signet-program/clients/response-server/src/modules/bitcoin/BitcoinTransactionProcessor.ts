import * as bitcoin from 'bitcoinjs-lib';
import type { ServerConfig } from '../../types';

export interface BitcoinInputSigningPlan {
  inputIndex: number;
  sighash: Uint8Array;
  prevTxid: string;
  vout: number;
}

export interface BitcoinSigningPlan {
  /**
   * Explorer-facing txid (Transaction.getId). Always the hex string you would
   * paste into a block explorer; never the little-endian buffer that
   * bitcoinjs-lib exposes internally.
   */
  explorerTxid: string;
  inputs: BitcoinInputSigningPlan[];
}

/**
 * Pre-compute signing material for each PSBT input.
 *
 * The MPC signer derives the per-input BIP-143 digest once, responds one
 * signature per request ID, and never needs to mutate the PSBT afterwards.
 */
export class BitcoinTransactionProcessor {
  /**
   * Parse a PSBT and pre-compute BIP-143 sighashes for each SegWit input.
   *
   * - Validates the network (testnet/regtest) from server config.
   * - Enforces SegWit v0 P2WPKH inputs with `witnessUtxo` metadata.
   * - Builds an explorer-facing txid (big-endian) plus per-input sighash data.
   *
   * @param psbtBytes Raw PSBT bytes from the Solana event.
   * @param config Server config, specifically `bitcoinNetwork` for network selection.
   * @returns Explorer txid and per-input signing material (sighash, prevout ref, index).
   */
  static createSigningPlan(
    psbtBytes: Uint8Array,
    config: ServerConfig
  ): BitcoinSigningPlan {
    const network = this.getNetwork(config);
    const psbt = bitcoin.Psbt.fromBuffer(Buffer.from(psbtBytes), { network });
    /**
     * In bitcoinjs-lib v6 the unsigned transaction inside a PSBT is a
     * `PsbtTransaction` (no hash helpers). Convert it to a full
     * `bitcoin.Transaction` via its `toBuffer()` method so we can call
     * `hashForWitnessV0` idiomatically without peeking at private caches.
     */

    const unsignedTxBuffer = psbt.data.globalMap.unsignedTx.toBuffer();
    const unsignedTx = bitcoin.Transaction.fromBuffer(unsignedTxBuffer);
    const txid = unsignedTx.getId(); 

    console.log(`🔍 Server txid from getId(): ${unsignedTx.getId()}`);

    console.log(
      `🔍 Server prevout from PSBT: ${psbt.txInputs[0].hash.toString('hex')}`
    );

    console.log(
      `🔐 Bitcoin PSBT: ${psbt.data.inputs.length} input(s), ${psbt.data.outputs.length} output(s)`
    );

    const inputs: BitcoinInputSigningPlan[] = [];
    for (let i = 0; i < psbt.data.inputs.length; i++) {
      const inputData = psbt.data.inputs[i];
      const witnessUtxo = inputData.witnessUtxo;
      if (!witnessUtxo) {
        throw new Error(
          `Input ${i} missing witnessUtxo (required for SegWit signing)`
        );
      }

      // Only SegWit v0 P2WPKH is supported; derive the legacy P2PKH
      // scriptCode required by BIP-143.
      const script = witnessUtxo.script;
      const isP2wpkhV0 =
        script.length === 22 && script[0] === 0x00 && script[1] === 0x14;

      if (!isP2wpkhV0) {
        const scriptHex = Buffer.from(script).toString('hex');
        throw new Error(
          `Input ${i} must be SegWit v0 P2WPKH (got script ${scriptHex})`
        );
      }

      const scriptCode = bitcoin.payments.p2pkh({
        hash: script.slice(2),
        network,
      }).output!;

      const sighashType =
        inputData.sighashType ?? bitcoin.Transaction.SIGHASH_ALL;

      const sighash = unsignedTx.hashForWitnessV0(
        i,
        scriptCode,
        witnessUtxo.value,
        sighashType
      );

      console.log(
        `🔍 Input ${i} sighash: ${Buffer.from(sighash).toString('hex')}`
      );
      console.log(`🔍 Input ${i} witnessUtxo.value: ${witnessUtxo.value}`);
      console.log(
        `🔍 Input ${i} witnessUtxo.script: ${witnessUtxo.script.toString('hex')}`
      );

      const prevTxid = Buffer.from(psbt.txInputs[i].hash)
        .reverse()
        .toString('hex');

      const vout = psbt.txInputs[i].index;

      inputs.push({
        inputIndex: i,
        sighash: new Uint8Array(sighash),
        prevTxid,
        vout,
      });
    }

    return {
      explorerTxid: txid,
      inputs,
    };
  }

  private static getNetwork(config: ServerConfig): bitcoin.Network {
    switch (config.bitcoinNetwork) {
      case 'testnet':
        return bitcoin.networks.testnet;
      case 'regtest':
        return bitcoin.networks.regtest;
      default:
        throw new Error(
          `Unsupported Bitcoin network '${config.bitcoinNetwork}'. Only regtest and testnet are available.`
        );
    }
  }
}
