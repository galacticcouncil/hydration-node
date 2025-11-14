import { assert } from "chai";
import * as anchor from "@coral-xyz/anchor";
import { testSetup } from "../test-utils/testSetup";
import { createSignArgs } from "../test-utils/signingUtils";

describe("Respond Error tests", () => {
  const { provider, program } = testSetup();

  it("Can respond with single error", async () => {
    const signArgs = createSignArgs("CONFIG_TEST");

    const requestId = Array.from({ length: 32 }, (_, i) => i % 256);

    const errorResponse = {
      requestId,
      errorMessage: "Test error message",
    };

    let errorEventReceived = false;
    let capturedEvent: any = null;

    const listener = program.addEventListener(
      "signatureErrorEvent",
      (event) => {
        errorEventReceived = true;
        capturedEvent = event;
      }
    );

    try {
      const tx = await program.methods
        .respondError([errorResponse])
        .accounts({
          responder: provider.wallet.publicKey,
        })
        .rpc();

      await new Promise((resolve) => setTimeout(resolve, 1000));

      assert.ok(tx, "Transaction should succeed");

      assert.ok(errorEventReceived, "signatureErrorEvent should be emitted");
      assert.ok(capturedEvent, "Event data should be captured");

      assert.deepEqual(
        Array.from(capturedEvent.requestId),
        requestId,
        "Request ID should match"
      );
      assert.equal(
        capturedEvent.responder.toString(),
        provider.wallet.publicKey.toString(),
        "Responder should match"
      );
      assert.equal(
        capturedEvent.error,
        "Test error message",
        "Error message should match"
      );
    } finally {
      await program.removeEventListener(listener);
    }
  });

  it("Can respond with multiple errors", async () => {
    const requestId1 = Array.from({ length: 32 }, (_, i) => (i + 1) % 256);
    const requestId2 = Array.from({ length: 32 }, (_, i) => (i + 2) % 256);

    const errorResponses = [
      {
        requestId: requestId1,
        errorMessage: "First error message",
      },
      {
        requestId: requestId2,
        errorMessage: "Second error message",
      },
    ];

    const capturedEvents: any[] = [];

    const listener = program.addEventListener(
      "signatureErrorEvent",
      (event) => {
        capturedEvents.push(event);
      }
    );

    try {
      const tx = await program.methods
        .respondError(errorResponses)
        .accounts({
          responder: provider.wallet.publicKey,
        })
        .rpc();

      await new Promise((resolve) => setTimeout(resolve, 1500));

      assert.ok(tx, "Transaction should succeed");

      assert.equal(
        capturedEvents.length,
        2,
        "Two signatureErrorEvents should be emitted"
      );

      const event1 = capturedEvents.find(
        (e) => Array.from(e.requestId).join(",") === requestId1.join(",")
      );
      assert.ok(event1, "First error event should be found");
      assert.equal(
        event1.error,
        "First error message",
        "First error message should match"
      );
      assert.equal(
        event1.responder.toString(),
        provider.wallet.publicKey.toString(),
        "First responder should match"
      );

      const event2 = capturedEvents.find(
        (e) => Array.from(e.requestId).join(",") === requestId2.join(",")
      );
      assert.ok(event2, "Second error event should be found");
      assert.equal(
        event2.error,
        "Second error message",
        "Second error message should match"
      );
      assert.equal(
        event2.responder.toString(),
        provider.wallet.publicKey.toString(),
        "Second responder should match"
      );
    } finally {
      await program.removeEventListener(listener);
    }
  });

  it("Can respond with error containing special characters", async () => {
    const requestId = Array.from({ length: 32 }, (_, i) => (i + 100) % 256);

    const errorResponse = {
      requestId,
      errorMessage:
        "Error with special chars: àáâãäå æç èéêë ìíîï ñ òóôõö ùúûü ý 🚨⚠️💥",
    };

    let errorEventReceived = false;
    let capturedEvent: any = null;

    const listener = program.addEventListener(
      "signatureErrorEvent",
      (event) => {
        errorEventReceived = true;
        capturedEvent = event;
      }
    );

    try {
      const tx = await program.methods
        .respondError([errorResponse])
        .accounts({
          responder: provider.wallet.publicKey,
        })
        .rpc();

      await new Promise((resolve) => setTimeout(resolve, 1000));

      assert.ok(tx, "Transaction should succeed");

      assert.ok(errorEventReceived, "signatureErrorEvent should be emitted");
      assert.ok(capturedEvent, "Event data should be captured");

      assert.equal(
        capturedEvent.error,
        errorResponse.errorMessage,
        "Error message with special characters should match"
      );
    } finally {
      await program.removeEventListener(listener);
    }
  });

  it("Can handle empty error message", async () => {
    const requestId = Array.from({ length: 32 }, (_, i) => (i + 200) % 256);

    const errorResponse = {
      requestId,
      errorMessage: "",
    };

    let errorEventReceived = false;
    let capturedEvent: any = null;

    const listener = program.addEventListener(
      "signatureErrorEvent",
      (event) => {
        errorEventReceived = true;
        capturedEvent = event;
      }
    );

    try {
      const tx = await program.methods
        .respondError([errorResponse])
        .accounts({
          responder: provider.wallet.publicKey,
        })
        .rpc();

      await new Promise((resolve) => setTimeout(resolve, 1000));

      assert.ok(tx, "Transaction should succeed");

      assert.ok(errorEventReceived, "signatureErrorEvent should be emitted");

      assert.equal(
        capturedEvent.error,
        "",
        "Empty error message should be handled correctly"
      );
    } finally {
      await program.removeEventListener(listener);
    }
  });
});
