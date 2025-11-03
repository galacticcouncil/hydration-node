import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import { ISubmittableResult } from "@polkadot/types/types";
import { waitReady } from "@polkadot/wasm-crypto";
import { u8aToHex } from "@polkadot/util";
import { encodeAddress } from "@polkadot/keyring";
import { ethers } from "ethers";
import { SignetClient } from "./signet-client";
import { KeyDerivation } from "./key-derivation";

const ROOT_PUBLIC_KEY =
  "0x044eef776e4f257d68983e45b340c2e9546c5df95447900b6aadfec68fb46fdee257e26b8ba383ddba9914b33c60e869265f859566fff4baef283c54d821ca3b64";
const CHAIN_ID = "polkadot:2034";
const USDC_SEPOLIA = "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238";
const SEPOLIA_RPC =
  process.env.SEPOLIA_RPC ||
  "https://sepolia.infura.io/v3/6df51ccaa17f4e078325b5050da5a2dd";

function getPalletAccountId(): Uint8Array {
  const palletId = new TextEncoder().encode("py/erc20");
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
  timeoutMs: number = 60000 // 60 seconds
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

            // Check for dispatch errors
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
            console.log(`⚠️  ${label} marked as Invalid`);
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
        await new Promise((resolve) => setTimeout(resolve, 2000)); // Wait 2s before retry
        continue;
      }
      throw error;
    }
  }

  throw new Error(`${label} failed after ${maxRetries + 1} attempts`);
}

describe("ERC20 Vault Integration", () => {
  let api: ApiPromise;
  let alice: any;
  let signetClient: SignetClient;
  let sepoliaProvider: ethers.JsonRpcProvider;
  let derivedEthAddress: string;
  let derivedPubKey: string;

  beforeAll(async () => {
    await waitReady();

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
      console.log(`Funding ERC20 vault pallet account ${palletSS58}...`);

      const fundTx = api.tx.balances.transferKeepAlive(
        palletSS58,
        fundingAmount
      );
      await submitWithRetry(fundTx, alice, api, "Fund pallet account");
    }

    signetClient = new SignetClient(api, alice);
    sepoliaProvider = new ethers.JsonRpcProvider(SEPOLIA_RPC);

    await signetClient.ensureInitialized(CHAIN_ID);

    const aliceAccountId = keyring.decodeAddress(alice.address);
    const aliceHexPath = "0x" + u8aToHex(aliceAccountId).slice(2);

    derivedPubKey = KeyDerivation.derivePublicKey(
      ROOT_PUBLIC_KEY,
      palletSS58,
      aliceHexPath,
      CHAIN_ID
    );

    derivedEthAddress = ethAddressFromPubKey(derivedPubKey);

    console.log(`\n🔑 Derived Ethereum Address: ${derivedEthAddress}`);
    await checkFunding();
  }, 120000);

  afterAll(async () => {
    if (api) {
      await api.disconnect();
    }
  });

  async function checkFunding() {
    const ethBalance = await sepoliaProvider.getBalance(derivedEthAddress);

    const usdcContract = new ethers.Contract(
      USDC_SEPOLIA,
      ["function balanceOf(address) view returns (uint256)"],
      sepoliaProvider
    );
    const usdcBalance = await usdcContract.balanceOf(derivedEthAddress);

    const feeData = await sepoliaProvider.getFeeData();
    const gasLimit = 100000n;
    const estimatedGas = (feeData.maxFeePerGas || 30000000000n) * gasLimit;

    console.log(`💰 Balances for ${derivedEthAddress}:`);
    console.log(`   ETH: ${ethers.formatEther(ethBalance)}`);
    console.log(`   USDC: ${ethers.formatUnits(usdcBalance, 6)}`);
    console.log(
      `   Estimated gas needed: ${ethers.formatEther(estimatedGas)} ETH\n`
    );

    const minUSDC = ethers.parseUnits("0.01", 6);

    if (ethBalance < estimatedGas) {
      throw new Error(
        `❌ Insufficient ETH at ${derivedEthAddress}\n` +
          `   Need: ${ethers.formatEther(estimatedGas)} ETH\n` +
          `   Have: ${ethers.formatEther(ethBalance)} ETH\n` +
          `   Please fund this address with ETH for gas`
      );
    }

    if (usdcBalance < minUSDC) {
      throw new Error(
        `❌ Insufficient USDC at ${derivedEthAddress}\n` +
          `   Need: 0.01 USDC\n` +
          `   Have: ${ethers.formatUnits(usdcBalance, 6)} USDC\n` +
          `   Please fund this address with USDC`
      );
    }
  }

  it("should complete full deposit and claim flow", async () => {
    const mpcEthAddress = ethAddressFromPubKey(ROOT_PUBLIC_KEY);
    console.log("Checking vault initialization...");
    const mpcAddressBytes = Array.from(ethers.getBytes(mpcEthAddress));

    const existingConfig = await api.query.erc20Vault.vaultConfig();
    const configJson = existingConfig.toJSON();

    if (configJson !== null) {
      console.log("⚠️  Vault already initialized, skipping initialization");
      console.log("   Existing config:", existingConfig.toHuman());
    } else {
      console.log("Initializing vault with MPC address:", mpcEthAddress);
      const initTx = api.tx.erc20Vault.initialize(mpcAddressBytes);
      await submitWithRetry(initTx, alice, api, "Initialize vault");
    }

    const amount = ethers.parseUnits("0.01", 6);
    const feeData = await sepoliaProvider.getFeeData();
    const currentNonce = await sepoliaProvider.getTransactionCount(
      derivedEthAddress,
      "pending"
    );

    console.log(`📊 Current nonce for ${derivedEthAddress}: ${currentNonce}`);

    const txParams = {
      value: 0,
      gasLimit: 100000,
      maxFeePerGas: Number(feeData.maxFeePerGas || 30000000000n),
      maxPriorityFeePerGas: Number(feeData.maxPriorityFeePerGas || 2000000000n),
      nonce: currentNonce,
      chainId: 11155111,
    };

    const keyring = new Keyring({ type: "sr25519" });
    const palletAccountId = getPalletAccountId();
    const palletSS58 = encodeAddress(palletAccountId, 0);
    const aliceAccountId = keyring.decodeAddress(alice.address);
    const aliceHexPath = "0x" + u8aToHex(aliceAccountId).slice(2);

    // Build transaction to get request ID
    const iface = new ethers.Interface([
      "function transfer(address to, uint256 amount) returns (bool)",
    ]);
    const data = iface.encodeFunctionData("transfer", [
      "0x00A40C2661293d5134E53Da52951A3F7767836Ef",
      amount,
    ]);

    const tx = ethers.Transaction.from({
      type: 2,
      chainId: txParams.chainId,
      nonce: txParams.nonce,
      maxPriorityFeePerGas: txParams.maxPriorityFeePerGas,
      maxFeePerGas: txParams.maxFeePerGas,
      gasLimit: txParams.gasLimit,
      to: USDC_SEPOLIA,
      value: 0,
      data: data,
    });

    const requestId = signetClient.calculateSignRespondRequestId(
      palletSS58,
      Array.from(ethers.getBytes(tx.unsignedSerialized)),
      {
        caip2Id: "eip155:11155111",
        keyVersion: 0,
        path: aliceHexPath,
        algo: "ecdsa",
        dest: "ethereum",
        params: "",
      }
    );

    console.log(`📋 Request ID: ${ethers.hexlify(requestId)}\n`);

    const requestIdBytes =
      typeof requestId === "string" ? ethers.getBytes(requestId) : requestId;

    const depositTx = api.tx.erc20Vault.depositErc20(
      Array.from(requestIdBytes),
      Array.from(ethers.getBytes(USDC_SEPOLIA)),
      amount.toString(),
      txParams
    );

    console.log("🚀 Submitting deposit_erc20 transaction...");
    const depositResult = await submitWithRetry(
      depositTx,
      alice,
      api,
      "Deposit ERC20"
    );

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
    } else {
      console.log("⚠️  No SignBidirectionalRequested event found!");
    }

    console.log("⏳ Waiting for MPC signature...");

    const signature = await signetClient.waitForSignature(
      ethers.hexlify(requestId),
      120000
    );

    if (!signature) {
      throw new Error("❌ Timeout waiting for MPC signature");
    }

    console.log(`✅ Received signature from: ${signature.responder}\n`);

    // Verify signature by recovering address
    const signedTx = constructSignedTransaction(
      tx.unsignedSerialized,
      signature.signature
    );
    const recoveredTx = ethers.Transaction.from(signedTx);
    const recoveredAddress = recoveredTx.from;

    console.log(`🔍 Signature verification:`);
    console.log(`   Expected address: ${derivedEthAddress}`);
    console.log(`   Recovered address: ${recoveredAddress}`);
    console.log(
      `   Match: ${
        recoveredAddress?.toLowerCase() === derivedEthAddress.toLowerCase()
      }`
    );

    if (recoveredAddress?.toLowerCase() !== derivedEthAddress.toLowerCase()) {
      throw new Error(
        `❌ Signature verification failed!\n` +
          `   Expected: ${derivedEthAddress}\n` +
          `   Recovered: ${recoveredAddress}\n` +
          `   This means the MPC signed with the wrong key or recovery ID is incorrect.`
      );
    }

    // Get fresh nonce before broadcasting
    const freshNonce = await sepoliaProvider.getTransactionCount(
      derivedEthAddress,
      "pending"
    );
    console.log(`📊 Fresh nonce check: ${freshNonce}`);

    if (freshNonce !== txParams.nonce) {
      throw new Error(
        `❌ Nonce mismatch! Expected ${txParams.nonce}, but network shows ${freshNonce}.\n` +
          `   A transaction may have already been sent from this address.`
      );
    }

    console.log("📡 Broadcasting transaction to Sepolia...");
    const txResponse = await sepoliaProvider.broadcastTransaction(signedTx);
    console.log(`   Tx Hash: ${txResponse.hash}`);

    const receipt = await txResponse.wait();
    console.log(`✅ Transaction confirmed in block ${receipt?.blockNumber}\n`);

    console.log("⏳ Waiting for MPC to read transaction result...");
    const readResponse = await waitForReadResponse(
      api,
      ethers.hexlify(requestId),
      120000
    );

    if (!readResponse) {
      throw new Error("❌ Timeout waiting for read response");
    }

    console.log("✅ Received read response\n");

    console.log("\n🔍 Claim Debug:");
    console.log("  Request ID:", ethers.hexlify(requestIdBytes));
    console.log(
      "  Output (hex):",
      Buffer.from(readResponse.output).toString("hex")
    );

    // Strip SCALE compact prefix from output
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

    const balanceBefore = await api.query.erc20Vault.userBalances(
      alice.address,
      Array.from(ethers.getBytes(USDC_SEPOLIA))
    );

    const claimTx = api.tx.erc20Vault.claimErc20(
      Array.from(requestIdBytes),
      Array.from(outputBytes),
      readResponse.signature
    );

    console.log("🚀 Submitting claim transaction...");
    await submitWithRetry(claimTx, alice, api, "Claim ERC20");

    const balanceAfter = await api.query.erc20Vault.userBalances(
      alice.address,
      Array.from(ethers.getBytes(USDC_SEPOLIA))
    );

    const balanceIncrease =
      BigInt(balanceAfter.toString()) - BigInt(balanceBefore.toString());

    expect(balanceIncrease.toString()).toBe(amount.toString());
    console.log(
      `✅ Balance increased by: ${ethers.formatUnits(
        balanceIncrease.toString(),
        6
      )} USDC`
    );
    console.log(
      `   Total balance: ${ethers.formatUnits(
        balanceAfter.toString(),
        6
      )} USDC\n`
    );
  }, 180000);

  function constructSignedTransaction(
    unsignedSerialized: string,
    signature: any
  ): string {
    const tx = ethers.Transaction.from(unsignedSerialized);

    const rHex = ethers.hexlify(signature.bigR.x);
    const sHex = ethers.hexlify(signature.s);

    tx.signature = {
      r: rHex,
      s: sHex,
      v: signature.recoveryId,
    };

    return tx.serialized;
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

  function ethAddressFromPubKey(pubKey: string): string {
    const hash = ethers.keccak256("0x" + pubKey.slice(4));
    return "0x" + hash.slice(-40);
  }
});
