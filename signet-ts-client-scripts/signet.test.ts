import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import { waitReady } from "@polkadot/wasm-crypto";
import { u8aToHex } from "@polkadot/util";
import { encodeAddress } from "@polkadot/keyring";
import { ethers } from "ethers";
import * as crypto from "crypto";
import { SignetClient } from "./signet-client";
import { KeyDerivation } from "./key-derivation";
import { TransactionBuilder } from "./transaction-builder";

describe("Signet Pallet Integration", () => {
  let api: ApiPromise;
  let client: SignetClient;
  let alice: any;
  let alicePolkadotAddress: string;
  
  const ROOT_PUBLIC_KEY = "0x044eef776e4f257d68983e45b340c2e9546c5df95447900b6aadfec68fb46fdee257e26b8ba383ddba9914b33c60e869265f859566fff4baef283c54d821ca3b64";
  const CHAIN_ID = "polkadot:2034";

  beforeAll(async () => {
    await waitReady();
    
    api = await ApiPromise.create({
      provider: new WsProvider("ws://127.0.0.1:8000"),
      types: {
        AffinePoint: { x: "[u8; 32]", y: "[u8; 32]" },
        Signature: { big_r: "AffinePoint", s: "[u8; 32]", recovery_id: "u8" }
      }
    });

    const keyring = new Keyring({ type: "sr25519" });
    alice = keyring.addFromUri("//Alice");
    alicePolkadotAddress = encodeAddress(alice.publicKey, 0);
    
    client = new SignetClient(api, alice);
    await client.ensureInitialized(CHAIN_ID);
  });

  afterAll(async () => {
    await api.disconnect();
  });

  describe("Sign", () => {
    it("should request and verify a signature", async () => {
      const payload = crypto.randomBytes(32);
      const params = {
        keyVersion: 1,
        path: "testPath",
        algo: "ecdsa",
        dest: "",
        params: "{}"
      };

      const requestId = client.calculateRequestId(alicePolkadotAddress, payload, params, CHAIN_ID);
      const derivedKey = KeyDerivation.derivePublicKey(ROOT_PUBLIC_KEY, alicePolkadotAddress, params.path, CHAIN_ID);
      
      await client.requestSignature(payload, params);
      
      const signature = await client.waitForSignature(requestId, 30000);
      expect(signature).toBeDefined();
      expect(signature.responder).toBeTruthy();
      
      console.log("\n    ✅ Signature received from:", signature.responder);
      
      const isValid = await client.verifySignature(payload, signature.signature, derivedKey);
      expect(isValid).toBe(true);
      
      console.log("    ✅ Signature verification PASSED");
    });
  });

  describe("SignRespond", () => {
    it("should request and verify a transaction signature", async () => {
      const tx = TransactionBuilder.buildEIP1559({
        chainId: 11155111,
        nonce: 0,
        maxPriorityFeePerGas: BigInt("2000000000"),
        maxFeePerGas: BigInt("30000000000"),
        gasLimit: 10000,
        to: "0x0000000000000000000000000000000000000000",
        value: BigInt(0),
        data: "0x",
        accessList: []
      });

      const params = {
        slip44ChainId: 60,
        keyVersion: 0,
        path: "testPath",
        schemas: {
          explorer: { format: 0, schema: "{}" },
          callback: { format: 0, schema: "{}" }
        }
      };

      const requestId = client.calculateSignRespondRequestId(
        alicePolkadotAddress, 
        tx.serialized, 
        params
      );
      
      const derivedKey = KeyDerivation.derivePublicKey(
        ROOT_PUBLIC_KEY, 
        alicePolkadotAddress, 
        params.path, 
        CHAIN_ID
      );
      
      await client.requestTransactionSignature(tx.serialized, params);
      
      const signature = await client.waitForSignature(requestId, 30000);
      expect(signature).toBeDefined();
      
      console.log("\n    ✅ Transaction signature received from:", signature.responder);
      
      const isValid = await client.verifyTransactionSignature(
        tx.transaction, 
        signature.signature, 
        derivedKey
      );
      expect(isValid).toBe(true);
      
      console.log("    ✅ Transaction signature verification PASSED");
    });
  });
});