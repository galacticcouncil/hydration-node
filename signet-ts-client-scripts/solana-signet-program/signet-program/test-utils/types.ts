import * as anchor from "@coral-xyz/anchor";

export interface SignatureRequestedEvent {
  sender: anchor.web3.PublicKey;
  payload: number[];
  keyVersion: number;
  path: string;
  algo: string;
  dest: string;
  params: string;
}
