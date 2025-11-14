import { assert } from "chai";
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ProxyTestCpi } from "../target/types/proxy_test_cpi";
import { testSetup } from "../test-utils/testSetup";
import {
  createSignArgs,
  callProxySign,
  waitForSignatureResponse,
  getPayloadDescription,
} from "../test-utils/signingUtils";

describe("Sign/Respond CPI tests", () => {
  const {
    provider,
    program: signetProgram,
    signetSolContract,
    evmChainAdapter,
    signatureRespondedSubscriber,
  } = testSetup();

  const proxyProgram = anchor.workspace.proxyTestCpi as Program<ProxyTestCpi>;

  const [eventAuthorityPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("__event_authority")],
    signetProgram.programId
  );

  it("Can call signet program via CPI and receive signature response", async () => {
    const signArgs = createSignArgs("CPI_TEST");

    await callProxySign(
      proxyProgram,
      signArgs,
      provider.wallet.publicKey,
      eventAuthorityPda
    );
    const response = await waitForSignatureResponse(
      signArgs,
      signetSolContract,
      evmChainAdapter,
      signatureRespondedSubscriber,
      provider.wallet.publicKey
    );

    assert.ok(response.isValid, "Signature should be valid");
  });

  it("Can handle multiple concurrent CPI calls", async () => {
    const signArgs1 = createSignArgs("CONCURRENT_TEST", "1", 1);
    const signArgs2 = createSignArgs("CONCURRENT_TEST", "2", 2);

    const [response1, response2] = await Promise.all([
      (async () => {
        await callProxySign(
          proxyProgram,
          signArgs1,
          provider.wallet.publicKey,
          eventAuthorityPda
        );
        return waitForSignatureResponse(
          signArgs1,
          signetSolContract,
          evmChainAdapter,
          signatureRespondedSubscriber,
          provider.wallet.publicKey
        );
      })(),
      (async () => {
        await callProxySign(
          proxyProgram,
          signArgs2,
          provider.wallet.publicKey,
          eventAuthorityPda
        );
        return waitForSignatureResponse(
          signArgs2,
          signetSolContract,
          evmChainAdapter,
          signatureRespondedSubscriber,
          provider.wallet.publicKey
        );
      })(),
    ]);

    assert.ok(response1.isValid, "First signature should be valid");
    assert.ok(response2.isValid, "Second signature should be valid");
  });
});
