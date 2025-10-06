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
  // Substrate's into_account_truncating: "modl" + pallet_id + padding
  const modl = new TextEncoder().encode("modl");
  const data = new Uint8Array(32);
  data.set(modl, 0);
  data.set(palletId, 4);
  return data;
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

      await new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          reject(new Error("Bob funding timeout"));
        }, 30000);

        api.tx.balances
          .transferKeepAlive(bob.address, 100000000000000n)
          .signAndSend(alice, (result: ISubmittableResult) => {
            if (result.dispatchError) {
              clearTimeout(timeout);
              reject(result.dispatchError);
            } else if (result.status.isInBlock) {
              // Changed from isFinalized
              clearTimeout(timeout);
              console.log("Bob's account funded!");
              resolve(result.status.asInBlock);
            }
          });
      });
    }

    signetClient = new SignetClient(api, alice);
    sepoliaProvider = new ethers.JsonRpcProvider(SEPOLIA_RPC);

    await signetClient.ensureInitialized(CHAIN_ID);

    const palletAccountId = getPalletAccountId();
    const palletSS58 = encodeAddress(palletAccountId, 0);

    const aliceAccountId = keyring.decodeAddress(alice.address);
    const aliceHexPath = "0x" + u8aToHex(aliceAccountId).slice(2);

    // Derive using PALLET account but ALICE's path
    derivedPubKey = KeyDerivation.derivePublicKey(
      ROOT_PUBLIC_KEY,
      palletSS58,
      aliceHexPath,
      CHAIN_ID
    );

    derivedEthAddress = ethAddressFromPubKey(derivedPubKey);

    console.log(`\n🔑 Derived Ethereum Address: ${derivedEthAddress}`);
    await checkFunding();
  }, 30000);

  afterAll(async () => {
    if (api) {
      await api.disconnect();
    }
  });

  async function checkFunding() {
    const ethBalance = await sepoliaProvider.getBalance(derivedEthAddress);

    // Check USDC balance
    const usdcContract = new ethers.Contract(
      USDC_SEPOLIA,
      ["function balanceOf(address) view returns (uint256)"],
      sepoliaProvider
    );
    const usdcBalance = await usdcContract.balanceOf(derivedEthAddress);

    // Estimate gas needed (conservative estimate)
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
    console.log("Initializing vault with MPC address:", mpcEthAddress);
    const mpcAddressBytes = Array.from(ethers.getBytes(mpcEthAddress));

    const initTx = api.tx.erc20Vault.initialize(mpcAddressBytes);
    await initTx.signAndSend(alice);
    await sleep(6000);

    console.log("✅ Vault initialized\n");

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
        slip44ChainId: 60,
        keyVersion: 0,
        path: aliceHexPath,
        algo: "ecdsa",
        dest: "ethereum",
        params: "",
      }
    );

    console.log(`📋 Request ID: ${ethers.hexlify(requestId)}\n`);

    // Convert requestId to bytes if it's a hex string
    const requestIdBytes =
      typeof requestId === "string" ? ethers.getBytes(requestId) : requestId;

    const depositTx = api.tx.erc20Vault.depositErc20(
      Array.from(requestIdBytes), // Use the bytes version
      Array.from(ethers.getBytes(USDC_SEPOLIA)),
      amount.toString(),
      txParams
    );

    console.log("🚀 Submitting deposit_erc20 transaction...");

    await new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(
          new Error(
            "Deposit transaction timeout - Chopsticks may have flaked, retry test"
          )
        );
      }, 15000);

      depositTx.signAndSend(alice, (result: ISubmittableResult) => {
        if (result.dispatchError) {
          clearTimeout(timeout);
          if (result.dispatchError.isModule) {
            const decoded = api.registry.findMetaError(
              result.dispatchError.asModule
            );
            reject(new Error(`${decoded.section}.${decoded.name}`));
          } else {
            reject(new Error(result.dispatchError.toString()));
          }
        } else if (result.status.isInBlock) {
          clearTimeout(timeout);
          resolve();
        }
      });
    });

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
        outputBytes = outputBytes.slice(1); // Remove 1-byte SCALE prefix
      } else if (mode === 1) {
        outputBytes = outputBytes.slice(2); // Remove 2-byte SCALE prefix
      } else if (mode === 2) {
        outputBytes = outputBytes.slice(4); // Remove 4-byte SCALE prefix
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

    await new Promise<void>((resolve, reject) => {
      claimTx.signAndSend(alice, (result: ISubmittableResult) => {
        if (result.dispatchError) {
          if (result.dispatchError.isModule) {
            const decoded = api.registry.findMetaError(
              result.dispatchError.asModule
            );
            reject(
              new Error(`Claim failed: ${decoded.section}.${decoded.name}`)
            );
          } else {
            reject(new Error(result.dispatchError.toString()));
          }
        } else if (result.status.isInBlock) {
          resolve();
        }
      });
    });

    console.log("✅ Claim transaction confirmed\n");

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
              event.method === "ReadResponded"
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

  function sleep(ms: number) {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
});
