import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import { ISubmittableResult } from "@polkadot/types/types";
import { waitReady } from "@polkadot/wasm-crypto";
import { u8aToHex } from "@polkadot/util";
import { encodeAddress } from "@polkadot/keyring";
import { ethers } from "ethers";
import * as bitcoin from "bitcoinjs-lib";
import Client from "bitcoin-core";
import { SignetClient } from "./signet-client";
import { KeyDerivation } from "./key-derivation";
import * as ecc from "tiny-secp256k1";
import coinSelect from "coinselect";

const ROOT_PUBLIC_KEY =
  "0x044eef776e4f257d68983e45b340c2e9546c5df95447900b6aadfec68fb46fdee257e26b8ba383ddba9914b33c60e869265f859566fff4baef283c54d821ca3b64";
const CHAIN_ID = "bip122:000000000933ea01ad0ee984209779ba";

function normalizeSignature(r: Buffer, s: Buffer): { r: Buffer; s: Buffer } {
  const N = BigInt(
    "0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141"
  );
  const halfN = N / 2n;
  const sBigInt = BigInt("0x" + s.toString("hex"));

  if (sBigInt > halfN) {
    const normalizedS = N - sBigInt;
    const sBuffer = Buffer.from(
      normalizedS.toString(16).padStart(64, "0"),
      "hex"
    );
    return { r, s: sBuffer };
  }

  return { r, s };
}

const TESTNET_VAULT_ADDRESS_HASH = new Uint8Array([
  0x89, 0xf0, 0xa8, 0x23, 0x93, 0x8c, 0x58, 0xcf, 0x5b, 0x17, 0xc8, 0xeb, 0x93,
  0xc6, 0x82, 0x80, 0x63, 0x5b, 0x73, 0x4e,
]);

function getPalletAccountId(): Uint8Array {
  const palletId = new TextEncoder().encode("py/btcvt");
  const modl = new TextEncoder().encode("modl");
  const data = new Uint8Array(32);
  data.set(modl, 0);
  data.set(palletId, 4);
  return data;
}

async function submitWithRetry(
  tx: any,
  signer: any,
  api: ApiPromise,
  label: string,
  maxRetries: number = 1,
  timeoutMs: number = 60000
): Promise<{ events: any[] }> {
  let attempt = 0;

  while (attempt <= maxRetries) {
    try {
      console.log(`${label} - Attempt ${attempt + 1}/${maxRetries + 1}`);

      const result = await new Promise<{ events: any[] }>((resolve, reject) => {
        let unsubscribe: any;

        const timer = setTimeout(() => {
          if (unsubscribe) unsubscribe();
          console.log(`⏱️  ${label} timed out after ${timeoutMs}ms`);
          reject(new Error("TIMEOUT"));
        }, timeoutMs);

        tx.signAndSend(signer, (result: ISubmittableResult) => {
          const { status, events, dispatchError } = result;

          if (status.isInBlock) {
            clearTimeout(timer);
            if (unsubscribe) unsubscribe();

            console.log(
              `✅ ${label} included in block ${status.asInBlock.toHex()}`
            );

            console.log(`📋 All events (${events.length}):`);
            for (const record of events) {
              const { event } = record;
              console.log(`   ${event.section}.${event.method}`);

              if (
                event.section === "btcVault" &&
                event.method === "DebugTxid"
              ) {
                const palletTxid = event.data[0].toU8a();
                console.log(
                  `🔍 PALLET TXID: ${Buffer.from(palletTxid).toString("hex")}`
                );
              }
            }

            if (dispatchError) {
              if (dispatchError.isModule) {
                const decoded = api.registry.findMetaError(
                  dispatchError.asModule
                );
                reject(
                  new Error(
                    `${decoded.section}.${decoded.name}: ${decoded.docs.join(
                      " "
                    )}`
                  )
                );
              } else {
                reject(new Error(dispatchError.toString()));
              }
              return;
            }

            resolve({ events: Array.from(events) });
          } else if (status.isInvalid) {
            clearTimeout(timer);
            if (unsubscribe) unsubscribe();
            reject(new Error("INVALID_TX"));
          } else if (status.isDropped) {
            clearTimeout(timer);
            if (unsubscribe) unsubscribe();
            reject(new Error(`${label} dropped`));
          }
        })
          .then((unsub: any) => {
            unsubscribe = unsub;
          })
          .catch((error: any) => {
            clearTimeout(timer);
            reject(error);
          });
      });

      return result;
    } catch (error: any) {
      if (
        (error.message === "INVALID_TX" || error.message === "TIMEOUT") &&
        attempt < maxRetries
      ) {
        console.log(`🔄 Retrying ${label}...`);
        attempt++;
        await new Promise((resolve) => setTimeout(resolve, 2000));
        continue;
      }
      throw error;
    }
  }

  throw new Error(`${label} failed after ${maxRetries + 1} attempts`);
}

describe("BTC Vault Integration", () => {
  let api: ApiPromise;
  let alice: any;
  let signetClient: SignetClient;
  let btcClient: any;
  let derivedBtcAddress: string;
  let derivedPubKey: string;
  let network: bitcoin.Network;

  beforeAll(async () => {
    await waitReady();

    console.log("🔗 Connecting to Bitcoin regtest...");
    btcClient = new Client({
      host: "http://localhost:18443",
      username: "test",
      password: "test123",
    });

    try {
      const blockCount = await btcClient.command("getblockcount");
      console.log(
        `✅ Connected to Bitcoin regtest (block height: ${blockCount})\n`
      );
    } catch (error) {
      throw new Error(
        "❌ Cannot connect to Bitcoin regtest. Make sure it's running:\n" +
          "   cd bitcoin-regtest && yarn docker:dev"
      );
    }

    api = await ApiPromise.create({
      provider: new WsProvider("ws://127.0.0.1:8000"),
      types: {
        AffinePoint: { x: "[u8; 32]", y: "[u8; 32]" },
        Signature: { big_r: "AffinePoint", s: "[u8; 32]", recovery_id: "u8" },
      },
    });

    const keyring = new Keyring({ type: "sr25519" });
    alice = keyring.addFromUri("//Alice");
    const bob = keyring.addFromUri("//Bob");

    const { data: bobBalance } = (await api.query.system.account(
      bob.address
    )) as any;

    if (bobBalance.free.toBigInt() < 1000000000000n) {
      console.log("Funding Bob's account for server responses...");
      const bobFundTx = api.tx.balances.transferKeepAlive(
        bob.address,
        100000000000000n
      );
      await submitWithRetry(bobFundTx, alice, api, "Fund Bob account");
    }

    const palletAccountId = getPalletAccountId();
    const palletSS58 = encodeAddress(palletAccountId, 0);

    const { data: palletBalance } = (await api.query.system.account(
      palletSS58
    )) as any;

    const fundingAmount = 10000000000000n;

    if (palletBalance.free.toBigInt() < fundingAmount) {
      console.log(`Funding BTC vault pallet account ${palletSS58}...`);
      const fundTx = api.tx.balances.transferKeepAlive(
        palletSS58,
        fundingAmount
      );
      await submitWithRetry(fundTx, alice, api, "Fund pallet account");
    }

    signetClient = new SignetClient(api, alice);
    await signetClient.ensureInitialized(CHAIN_ID);

    const aliceAccountId = keyring.decodeAddress(alice.address);
    const aliceHexPath = "0x" + u8aToHex(aliceAccountId).slice(2);

    derivedPubKey = KeyDerivation.derivePublicKey(
      ROOT_PUBLIC_KEY,
      palletSS58,
      aliceHexPath,
      "polkadot:2034"
    );

    const compressedForComparison = compressPubkey(derivedPubKey);
    console.log(
      `🔍 Test expects compressed pubkey: ${compressedForComparison.toString(
        "hex"
      )}`
    );

    console.log(`🔍 Test using path: ${aliceHexPath}`);
    console.log(`🔍 Test using sender: ${palletSS58}`);
    console.log(`🔍 Derived public key: ${derivedPubKey}`);

    network = bitcoin.networks.regtest;
    derivedBtcAddress = btcAddressFromPubKey(derivedPubKey, network);

    console.log(`\n🔑 Derived Bitcoin Address: ${derivedBtcAddress}`);

    await fundBitcoinAddress(derivedBtcAddress);
  }, 180000);

  afterAll(async () => {
    if (api) {
      await api.disconnect();
    }
  });

  async function fundBitcoinAddress(address: string) {
    console.log(`💰 Funding ${address} with 1 BTC...`);

    try {
      let walletAddress;
      try {
        walletAddress = await btcClient.command("getnewaddress");
      } catch (e) {
        try {
          await btcClient.command("createwallet", "testwallet");
          walletAddress = await btcClient.command("getnewaddress");
        } catch (createErr: any) {
          await btcClient.command("loadwallet", "testwallet");
          walletAddress = await btcClient.command("getnewaddress");
        }
      }

      await btcClient.command("generatetoaddress", 101, walletAddress);
      console.log(`   Mined 101 blocks to wallet`);

      const txid = await btcClient.command("sendtoaddress", address, 1.0);
      console.log(`   Funding txid: ${txid}`);

      await btcClient.command("generatetoaddress", 1, walletAddress);

      const scanResult = await btcClient.command("scantxoutset", "start", [
        `addr(${derivedBtcAddress})`,
      ]);

      console.log(`📦 Found ${scanResult.unspents.length} UTXO(s)`);

      for (const utxo of scanResult.unspents) {
        console.log(`\n🔍 UTXO ${utxo.txid}:${utxo.vout}`);
        console.log(`   scriptPubKey: ${utxo.scriptPubKey}`);
        console.log(`   amount: ${utxo.amount} BTC`);
      }

      if (scanResult.unspents.length === 0) {
        throw new Error("No UTXOs found after funding");
      }

      console.log("✅ Funding confirmed\n");
    } catch (error: any) {
      console.error("Funding error:", error.message);
      throw error;
    }
  }

  it("should complete full deposit and claim flow", async () => {
    const mpcEthAddress = ethAddressFromPubKey(ROOT_PUBLIC_KEY);
    console.log("Checking vault initialization...");

    const existingConfig = await api.query.btcVault.vaultConfig();
    const configJson = existingConfig.toJSON();

    if (configJson !== null) {
      console.log("⚠️  Vault already initialized, skipping initialization");
      console.log("   Existing config:", existingConfig.toHuman());
    } else {
      console.log("Initializing vault with MPC address hash");
      const initTx = api.tx.btcVault.initialize(Array.from(mpcEthAddress));
      await submitWithRetry(initTx, alice, api, "Initialize vault");
    }

    const scanResult = await btcClient.command("scantxoutset", "start", [
      `addr(${derivedBtcAddress})`,
    ]);

    if (scanResult.unspents.length === 0) {
      throw new Error("No UTXOs available for derived address");
    }

    console.log(`📦 Found ${scanResult.unspents.length} UTXO(s)`);

    const depositAmount = 36879690;
    const feeRate = 1; // satoshis per byte

    // Convert UTXOs to coinselect format
    const utxos = scanResult.unspents.map((u: any) => ({
      txid: u.txid,
      vout: u.vout,
      value: Math.floor(u.amount * 100000000),
      script: Buffer.from(
        bitcoin.address.toOutputScript(derivedBtcAddress, network)
      ),
    }));

    // Vault output
    const vaultScript = Buffer.concat([
      Buffer.from([0x00, 0x14]),
      Buffer.from(TESTNET_VAULT_ADDRESS_HASH),
    ]);

    const targets = [
      {
        script: vaultScript,
        value: depositAmount,
      },
    ];

    // Let coinselect pick optimal UTXOs and calculate fee
    const { inputs, outputs, fee } = coinSelect(utxos, targets, feeRate);

    if (!inputs || !outputs) {
      throw new Error("Insufficient funds for transaction");
    }

    console.log(`📊 Transaction breakdown:`);
    console.log(`   Inputs: ${inputs.length}`);
    inputs.forEach((inp: any, i: number) => {
      console.log(
        `     Input ${i}: ${inp.value} sats (${inp.txid.slice(0, 8)}...)`
      );
    });
    console.log(`   To vault: ${outputs[0].value} sats`);
    if (outputs.length > 1) {
      console.log(`   Change: ${outputs[1].value} sats`);
    }
    console.log(`   Fee: ${fee} sats\n`);

    // Build PSBT with selected inputs/outputs
    const psbt = new bitcoin.Psbt({ network });

    for (const input of inputs) {
      const txidBytes = Buffer.from(input.txid, "hex").reverse();
      psbt.addInput({
        hash: txidBytes,
        index: input.vout,
        sequence: 0xffffffff,
        witnessUtxo: {
          script: input.script!,
          value: input.value,
        },
      });
    }

    for (let i = 0; i < inputs.length; i++) {
      psbt.updateInput(i, {
        sighashType: bitcoin.Transaction.SIGHASH_ALL,
      });
    }

    for (const output of outputs) {
      if (output.script) {
        psbt.addOutput({
          script: output.script,
          value: output.value,
        });
      } else {
        // Change output
        psbt.addOutput({
          address: derivedBtcAddress,
          value: output.value,
        });
      }
    }

    const psbtBytes = psbt.toBuffer();
    console.log(`📝 PSBT bytes length: ${psbtBytes.length}`);

    console.log(`🔍 Test prevout txid (as sent to pallet): ${inputs[0].txid}`);
    console.log(
      `🔍 Test prevout txid (reversed for PSBT): ${Buffer.from(
        inputs[0].txid,
        "hex"
      )
        .reverse()
        .toString("hex")}`
    );

    // Extract txid from PSBT
    const unsignedTxBuffer = psbt.data.globalMap.unsignedTx.toBuffer();
    const unsignedTx = bitcoin.Transaction.fromBuffer(unsignedTxBuffer);
    const txid = Buffer.from(unsignedTx.getId(), "hex");

    const testUnsignedTx = bitcoin.Transaction.fromBuffer(unsignedTxBuffer);
    for (let i = 0; i < inputs.length; i++) {
      const testSighash = testUnsignedTx.hashForWitnessV0(
        i,
        inputs[i].script!,
        inputs[i].value,
        bitcoin.Transaction.SIGHASH_ALL
      );
      console.log(`🔍 Test Input ${i} sighash: ${testSighash.toString("hex")}`);
    }

    console.log(`🔑 Transaction ID: ${txid.toString("hex")}`);

    console.log(`🔍 Test tx version: ${unsignedTx.version}`);
    console.log(`🔍 Test tx locktime: ${unsignedTx.locktime}`);

    const keyring = new Keyring({ type: "sr25519" });
    const palletAccountId = getPalletAccountId();
    const palletSS58 = encodeAddress(palletAccountId, 0);
    const aliceAccountId = keyring.decodeAddress(alice.address);
    const aliceHexPath = "0x" + u8aToHex(aliceAccountId).slice(2);

    console.log(`🔍 Pallet SS58 (sender): ${palletSS58}`);
    console.log(`🔍 Alice hex path: ${aliceHexPath}`);

    // Calculate aggregate request ID (for monitoring)
    const aggregateRequestId = signetClient.calculateSignRespondRequestId(
      palletSS58,
      Array.from(txid),
      {
        caip2Id: CHAIN_ID,
        keyVersion: 0,
        path: aliceHexPath,
        algo: "ecdsa",
        dest: "bitcoin",
        params: "",
      }
    );

    console.log(
      `📋 Aggregate Request ID: ${ethers.hexlify(aggregateRequestId)}`
    );

    // Generate per-input request IDs
    const txidForPerInputRequestId = Buffer.from(unsignedTx.getId(), "hex");

    // Generate per-input request IDs
    const perInputRequestIds: string[] = [];
    for (let i = 0; i < inputs.length; i++) {
      const inputIndexBytes = Buffer.alloc(4);
      inputIndexBytes.writeUInt32LE(i, 0);
      // Use the display order txid reversed (internal order) to match server
      const txDataForInput = Buffer.concat([
        txidForPerInputRequestId,
        inputIndexBytes,
      ]);

      const perInputRequestId = signetClient.calculateSignRespondRequestId(
        palletSS58,
        Array.from(txDataForInput),
        {
          caip2Id: CHAIN_ID,
          keyVersion: 0,
          path: aliceHexPath,
          algo: "ecdsa",
          dest: "bitcoin",
          params: "",
        }
      );

      perInputRequestIds.push(ethers.hexlify(perInputRequestId));
      console.log(`📋 Input ${i} Request ID: ${perInputRequestIds[i]}`);
    }
    console.log(""); // Empty line

    // Convert to pallet format
    const palletInputs = inputs.map((input: any) => ({
      txid: Array.from(Buffer.from(input.txid, "hex")),
      vout: input.vout,
      value: input.value,
      scriptPubkey: Array.from(input.script),
      sequence: 0xffffffff,
    }));

    const palletOutputs = outputs.map((output: any) => {
      if (output.script) {
        return {
          value: output.value,
          scriptPubkey: Array.from(output.script),
        };
      } else {
        return {
          value: output.value,
          scriptPubkey: Array.from(
            bitcoin.address.toOutputScript(derivedBtcAddress, network)
          ),
        };
      }
    });

    const depositTx = api.tx.btcVault.depositBtc(
      Array.from(ethers.getBytes(aggregateRequestId)),
      palletInputs,
      palletOutputs,
      0
    );

    console.log("🚀 Submitting deposit_btc transaction...");
    const depositResult = await submitWithRetry(
      depositTx,
      alice,
      api,
      "Deposit BTC"
    );

    const debugEvent = depositResult.events.find(
      (record: any) =>
        record.event.section === "btcVault" &&
        record.event.method === "DebugTxid"
    );

    if (debugEvent) {
      const palletTxid = debugEvent.event.data[0].toU8a();
      console.log(
        `🔍 Pallet computed txid: ${Buffer.from(palletTxid).toString("hex")}`
      );
      console.log(
        `🔍 Test computed txid:   ${Buffer.from(txid).toString("hex")}`
      );
      console.log(
        `🔍 Match: ${
          Buffer.from(palletTxid).toString("hex") ===
          Buffer.from(txid).toString("hex")
        }`
      );
    } else {
      console.log("⚠️  DebugTxid event not found");
    }

    const debugTxEvent = depositResult.events.find(
      (record: any) =>
        record.event.section === "btcVault" &&
        record.event.method === "DebugTransaction"
    );

    if (debugTxEvent) {
      const palletTxHex = debugTxEvent.event.data[0].toHex();
      const palletVersion = debugTxEvent.event.data[1].toNumber();
      const palletLocktime = debugTxEvent.event.data[2].toNumber();

      console.log(`🔍 Pallet PSBT hex: ${palletTxHex}`);
      console.log(
        `🔍 Pallet version: ${palletVersion}, locktime: ${palletLocktime}`
      );
    }

    const signetEvents = depositResult.events.filter(
      (record: any) =>
        record.event.section === "signet" &&
        record.event.method === "SignBidirectionalRequested"
    );

    console.log(
      `📊 Found ${signetEvents.length} SignBidirectionalRequested event(s)`
    );

    if (signetEvents.length > 0) {
      console.log(
        "✅ SignBidirectionalRequested event emitted - MPC should pick it up!"
      );
    }

    console.log("⏳ Waiting for MPC signature(s)...");

    // Wait for each input's signature separately
    const signatures: any[] = [];
    for (let i = 0; i < perInputRequestIds.length; i++) {
      console.log(
        `   Waiting for signature ${i + 1}/${perInputRequestIds.length}...`
      );
      const sig = await waitForSingleSignature(
        api,
        perInputRequestIds[i],
        120000
      );
      signatures.push(sig);
      console.log(`   ✅ Received signature ${i + 1}`);
    }

    console.log(
      `\n✅ Received all ${signatures.length} signature(s) from MPC\n`
    );

    const compressedPubkey = compressPubkey(derivedPubKey);

    for (let i = 0; i < signatures.length; i++) {
      const sig = signatures[i];

      const rBuf =
        typeof sig.bigR.x === "string"
          ? Buffer.from(sig.bigR.x.slice(2), "hex")
          : Buffer.from(sig.bigR.x);

      const sBuf =
        typeof sig.s === "string"
          ? Buffer.from(sig.s.slice(2), "hex")
          : Buffer.from(sig.s);

      const { r: normalizedR, s: normalizedS } = normalizeSignature(rBuf, sBuf);

      console.log(`🔍 Signature ${i} verification:`);
      console.log(`   R: ${normalizedR.toString("hex")}`);
      console.log(`   S: ${normalizedS.toString("hex")}`);
      console.log(`   Recovery ID: ${sig.recoveryId}`);

      // VERIFY THE SIGNATURE
      const testSighash = testUnsignedTx.hashForWitnessV0(
        i,
        inputs[i].script!,
        inputs[i].value,
        bitcoin.Transaction.SIGHASH_ALL
      );

      // Verify signature is valid
      const rawSig = Buffer.concat([normalizedR, normalizedS]);
      const isValid = ecc.verify(testSighash, compressedPubkey, rawSig);
      console.log(`   Signature valid (ecc.verify): ${isValid}`);

      const derSig = encodeDER(normalizedR, normalizedS);
      const fullSig = Buffer.concat([derSig, Buffer.from([0x01])]);

      psbt.updateInput(i, {
        partialSig: [
          {
            pubkey: compressedPubkey,
            signature: fullSig,
          },
        ],
      });
    }

    console.log(`🔍 Input 0 witnessUtxo:`, psbt.data.inputs[0].witnessUtxo);
    psbt.finalizeAllInputs();
    const signedTx = psbt.extractTransaction();
    console.log(`🔍 Witness for input 0:`);
    const witness = signedTx.ins[0].witness;
    witness.forEach((w, idx) => {
      console.log(`   Witness ${idx}: ${w.toString("hex")}`);
    });
    const signedTxHex = signedTx.toHex();

    console.log(`\n🔍 Full signed transaction hex (first 200 chars):`);
    console.log(`   ${signedTxHex.substring(0, 200)}...`);
    console.log(
      `🔍 Transaction input 0 sequence: 0x${signedTx.ins[0].sequence.toString(
        16
      )}`
    );

    console.log(`✅ Transaction finalized: ${signedTx.getId()}\n`);

    console.log("📡 Broadcasting transaction to regtest...");
    const broadcastTxid = await btcClient.command(
      "sendrawtransaction",
      signedTxHex
    );
    console.log(`   Tx Hash: ${broadcastTxid}`);

    console.log("⛏️  Mining block to confirm transaction...");
    await btcClient.command("generatetoaddress", 1, derivedBtcAddress);

    const txDetails = await btcClient.command(
      "getrawtransaction",
      broadcastTxid,
      true
    );

    console.log(
      `✅ Transaction confirmed (${txDetails.confirmations} confirmations)\n`
    );

    console.log("⏳ Waiting for MPC to read transaction result...");
    const readResponse = await waitForReadResponse(
      api,
      ethers.hexlify(aggregateRequestId),
      120000
    );

    if (!readResponse) {
      throw new Error("❌ Timeout waiting for read response");
    }

    console.log("✅ Received read response\n");

    console.log("\n🔍 Claim Debug:");
    console.log("  Request ID:", ethers.hexlify(aggregateRequestId));
    console.log(
      "  Output (hex):",
      Buffer.from(readResponse.output).toString("hex")
    );

    let outputBytes = new Uint8Array(readResponse.output);
    if (outputBytes.length > 0) {
      const mode = outputBytes[0] & 0b11;
      if (mode === 0) {
        outputBytes = outputBytes.slice(1);
      } else if (mode === 1) {
        outputBytes = outputBytes.slice(2);
      } else if (mode === 2) {
        outputBytes = outputBytes.slice(4);
      }
    }

    console.log(
      "  Stripped output (hex):",
      Buffer.from(outputBytes).toString("hex")
    );

    const balanceBefore = await api.query.btcVault.userBalances(alice.address);

    const claimTx = api.tx.btcVault.claimBtc(
      Array.from(ethers.getBytes(aggregateRequestId)),
      Array.from(outputBytes),
      readResponse.signature
    );

    console.log("🚀 Submitting claim transaction...");
    await submitWithRetry(claimTx, alice, api, "Claim BTC");

    const balanceAfter = await api.query.btcVault.userBalances(alice.address);

    const balanceIncrease =
      BigInt(balanceAfter.toString()) - BigInt(balanceBefore.toString());

    expect(balanceIncrease.toString()).toBe(depositAmount.toString());
    console.log(`✅ Balance increased by: ${balanceIncrease} sats`);
    console.log(`   Total balance: ${balanceAfter} sats\n`);
  }, 300000);

  function compressPubkey(pubKey: string): Buffer {
    const uncompressed = Buffer.from(pubKey.slice(4), "hex");
    const fullUncompressed = Buffer.concat([Buffer.from([0x04]), uncompressed]);
    const compressed = ecc.pointCompress(fullUncompressed, true);
    return Buffer.from(compressed);
  }

  function btcAddressFromPubKey(
    pubKey: string,
    network: bitcoin.Network
  ): string {
    const compressedPubkey = compressPubkey(pubKey);
    const payment = bitcoin.payments.p2wpkh({
      pubkey: compressedPubkey,
      network,
    });
    return payment.address!;
  }

  function ethAddressFromPubKey(pubKey: string): Uint8Array {
    const uncompressedPubkey = Buffer.from(pubKey.slice(4), "hex");
    const hash = ethers.keccak256(uncompressedPubkey);
    return new Uint8Array(Buffer.from(hash.slice(2), "hex").slice(-20));
  }

  function encodeDER(r: Buffer, s: Buffer): Buffer {
    function toDER(x: Buffer): Buffer {
      let i = 0;
      while (i < x.length - 1 && x[i] === 0 && x[i + 1] < 0x80) i++;

      const xDER = x.slice(i);

      if (xDER[0] >= 0x80) {
        return Buffer.concat([Buffer.from([0x00]), xDER]);
      }

      return xDER;
    }

    const rDER = toDER(r);
    const sDER = toDER(s);

    const len = 2 + rDER.length + 2 + sDER.length;
    const buf = Buffer.allocUnsafe(2 + len);

    buf[0] = 0x30;
    buf[1] = len;
    buf[2] = 0x02;
    buf[3] = rDER.length;
    rDER.copy(buf, 4);
    buf[4 + rDER.length] = 0x02;
    buf[5 + rDER.length] = sDER.length;
    sDER.copy(buf, 6 + rDER.length);

    return buf;
  }

  async function waitForSingleSignature(
    api: ApiPromise,
    requestId: string,
    timeout: number
  ): Promise<any> {
    return new Promise((resolve, reject) => {
      let unsubscribe: any;
      const timer = setTimeout(() => {
        if (unsubscribe) unsubscribe();
        reject(
          new Error(
            `Timeout waiting for signature with request ID ${requestId}`
          )
        );
      }, timeout);

      api.query.system
        .events((events: any) => {
          events.forEach((record: any) => {
            const { event } = record;
            if (
              event.section === "signet" &&
              event.method === "SignatureResponded"
            ) {
              const [reqId, responder, signature] = event.data;
              if (ethers.hexlify(reqId.toU8a()) === requestId) {
                clearTimeout(timer);
                if (unsubscribe) unsubscribe();
                resolve(signature.toJSON());
              }
            }
          });
        })
        .then((unsub: any) => {
          unsubscribe = unsub;
        });
    });
  }

  async function waitForReadResponse(
    api: ApiPromise,
    requestId: string,
    timeout: number
  ): Promise<any> {
    return new Promise((resolve) => {
      let unsubscribe: any;
      const timer = setTimeout(() => {
        if (unsubscribe) unsubscribe();
        resolve(null);
      }, timeout);

      api.query.system
        .events((events: any) => {
          events.forEach((record: any) => {
            const { event } = record;
            if (
              event.section === "signet" &&
              event.method === "RespondBidirectionalEvent"
            ) {
              const [reqId, responder, output, signature] = event.data;
              if (ethers.hexlify(reqId.toU8a()) === requestId) {
                clearTimeout(timer);
                if (unsubscribe) unsubscribe();
                resolve({
                  responder: responder.toString(),
                  output: Array.from(output.toU8a()),
                  signature: signature.toJSON(),
                });
              }
            }
          });
        })
        .then((unsub: any) => {
          unsubscribe = unsub;
        });
    });
  }
});
