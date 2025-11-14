import { assert } from "chai";
import { testSetup } from "../test-utils/testSetup";
import {
  createSignArgs,
  callDirectSign,
  waitForSignatureResponse,
  getPayloadDescription,
} from "../test-utils/signingUtils";

describe("Sign/Respond wallet tests", () => {
  const {
    provider,
    program,
    signetSolContract,
    evmChainAdapter,
    signatureRespondedSubscriber,
  } = testSetup();

  it("Can request a signature", async () => {
    const signArgs = createSignArgs("WALLET_TEST");

    await callDirectSign(program, signArgs);
    const response = await waitForSignatureResponse(
      signArgs,
      signetSolContract,
      evmChainAdapter,
      signatureRespondedSubscriber,
      provider.wallet.publicKey
    );

    assert.ok(response.isValid, "Signature should be valid");
  });
});
