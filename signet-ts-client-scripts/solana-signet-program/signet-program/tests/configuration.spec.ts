import * as anchor from "@coral-xyz/anchor";
import { BN } from "@coral-xyz/anchor";
import { assert } from "chai";
import { Keypair, PublicKey } from "@solana/web3.js";
import { testSetup } from "../test-utils/testSetup";
import { confirmTransaction } from "../test-utils/utils";
import {
  createSignArgs,
  callDirectSign,
  getPayloadDescription,
} from "../test-utils/signingUtils";

describe("Configuration Functions", () => {
  const { program, connection, provider } = testSetup();

  let programStatePda: PublicKey;
  let nonAdminKeypair: Keypair;
  let recipientKeypair: Keypair;

  const getEventsFromTransaction = async (txSignature: string) => {
    const tx = await connection.getTransaction(txSignature, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });

    if (!tx) throw new Error("Transaction not found");

    const eventParser = new anchor.EventParser(
      program.programId,
      program.coder
    );

    return Array.from(eventParser.parseLogs(tx.meta?.logMessages || []));
  };

  before(async () => {
    [programStatePda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("program-state")],
      program.programId
    );

    // Generate keypairs without funding - we only need their public keys for authorization tests
    nonAdminKeypair = Keypair.generate();
    recipientKeypair = Keypair.generate();
  });

  it("Is initialized", async () => {
    const [programStatePda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("program-state")],
      program.programId
    );

    const programState = await program.account.programState.fetch(
      programStatePda
    );

    const expectedDeposit = new BN("100000");
    assert.ok(
      programState.signatureDeposit.eq(expectedDeposit),
      `Expected deposit ${expectedDeposit.toString()}, got ${programState.signatureDeposit.toString()}`
    );

    assert.ok(
      programState.admin.equals(provider.wallet.publicKey),
      "Admin should be set to the wallet public key"
    );
  });

  describe("update_deposit", () => {
    it("Should successfully update deposit when called by admin", async () => {
      const newDeposit = new BN("200000");

      const txSignature = await program.methods.updateDeposit(newDeposit).rpc();
      await confirmTransaction(connection, txSignature);
      const events = await getEventsFromTransaction(txSignature);

      const programStateAfter = await program.account.programState.fetch(
        programStatePda
      );
      assert.ok(
        programStateAfter.signatureDeposit.eq(newDeposit),
        `Expected deposit ${newDeposit.toString()}, got ${programStateAfter.signatureDeposit.toString()}`
      );

      const depositUpdatedEvents = events.filter(
        (e) => e.name === "depositUpdatedEvent"
      );

      const eventData = depositUpdatedEvents[0].data;
      assert.ok(
        eventData.newDeposit.eq(newDeposit),
        "Event should contain new deposit"
      );
    });

    it("Should fail when called by non-admin", async () => {
      try {
        await program.methods
          .updateDeposit(new BN("300000"))
          .accounts({ admin: nonAdminKeypair.publicKey })
          .signers([nonAdminKeypair])
          .rpc();

        assert.fail("Should have thrown an error for unauthorized access");
      } catch (error) {
        assert.ok(
          error.message.includes("Unauthorized access"),
          `Expected unauthorized error, got: ${error.message}`
        );
      }
    });
  });

  describe("withdraw_funds", () => {
    const newDeposit = new BN("50000");

    beforeEach(async () => {
      const updateDepositTx = await program.methods
        .updateDeposit(newDeposit)
        .rpc();
      await confirmTransaction(connection, updateDepositTx);

      const programStateInfoBefore = await connection.getAccountInfo(
        programStatePda,
        "confirmed"
      );

      // Withdraw all existing funds to start with a clean state
      try {
        await program.account.programState.fetch(programStatePda);

        if (
          programStateInfoBefore.data &&
          programStateInfoBefore &&
          programStateInfoBefore.lamports > 0
        ) {
          const rentExemptAmount =
            await connection.getMinimumBalanceForRentExemption(
              programStateInfoBefore.data.length
            );
          const availableFunds = new BN(
            programStateInfoBefore.lamports - rentExemptAmount
          );

          if (availableFunds.gt(new BN(0))) {
            await program.methods
              .withdrawFunds(availableFunds)
              .accountsPartial({ recipient: provider.wallet.publicKey })
              .rpc();
          }
        }
      } catch (error) {
        // Program not initialized, skip withdrawal
      }

      const signArgs = createSignArgs("CONFIG_TEST", "deposit", 1);

      const signTx = await callDirectSign(program, signArgs);

      await confirmTransaction(connection, signTx);
    });

    it("Should successfully withdraw funds when called by admin", async () => {
      const recipient = provider.wallet.publicKey;

      const programStateInfoBefore = await connection.getAccountInfo(
        programStatePda,
        "confirmed"
      );

      if (!programStateInfoBefore) {
        throw new Error("Program state account not found");
      }

      const txSignature = await program.methods
        .withdrawFunds(newDeposit)
        .accountsPartial({ recipient })
        .rpc();

      await confirmTransaction(connection, txSignature);
      const events = await getEventsFromTransaction(txSignature);

      const programStateInfoAfter = await connection.getAccountInfo(
        programStatePda,
        "confirmed"
      );

      if (!programStateInfoAfter) {
        throw new Error("Program state account not found after withdrawal");
      }

      assert.ok(
        programStateInfoAfter.lamports ===
          programStateInfoBefore.lamports - newDeposit.toNumber(),
        "Program state should have less lamports"
      );

      const fundsWithdrawnEvents = events.filter(
        (e) => e.name === "fundsWithdrawnEvent"
      );
      assert.ok(
        fundsWithdrawnEvents.length > 0,
        "FundsWithdrawnEvent should have been emitted"
      );

      const eventData = fundsWithdrawnEvents[0].data;
      assert.ok(
        eventData.amount.eq(newDeposit),
        "Event should contain withdrawal amount"
      );
      assert.ok(
        eventData.recipient.equals(recipient),
        "Event should contain recipient"
      );
    });

    it("Should fail when called by non-admin", async () => {
      try {
        await program.methods
          .withdrawFunds(new BN("50000"))
          .accountsPartial({
            admin: nonAdminKeypair.publicKey,
            recipient: recipientKeypair.publicKey,
          })
          .signers([nonAdminKeypair])
          .rpc();

        assert.fail("Should have thrown an error for unauthorized access");
      } catch (error) {
        assert.ok(
          error.message.includes("Unauthorized access"),
          `Expected unauthorized error, got: ${error.message}`
        );
      }
    });

    it("Should fail when trying to withdraw more than available", async () => {
      const programStateInfo =
        (await connection.getAccountInfo(programStatePda)) ||
        (await program.provider.connection.getAccountInfo(programStatePda));

      if (!programStateInfo) {
        throw new Error("Program state account not found");
      }

      const excessiveAmount = new BN(programStateInfo.lamports + 1000000);

      try {
        await program.methods
          .withdrawFunds(excessiveAmount)
          .accountsPartial({ recipient: recipientKeypair.publicKey })
          .rpc();

        assert.fail("Should have thrown an error for insufficient funds");
      } catch (error) {
        assert.ok(
          error.message.includes("Insufficient funds for withdrawal"),
          `Expected insufficient funds error, got: ${error.message}`
        );
      }
    });

    it("Should fail when recipient is zero address", async () => {
      const withdrawAmount = new BN("50000");

      try {
        await program.methods
          .withdrawFunds(withdrawAmount)
          .accountsPartial({ recipient: PublicKey.default })
          .rpc();

        assert.fail("Should have thrown an error for invalid recipient");
      } catch (error) {
        // Caught by Anchor macro validation, error message can't be customized
        assert.ok(
          error.message.includes("A mut constraint was violated"),
          `Expected invalid recipient error, got: ${error.message}`
        );
      }
    });
  });
});
