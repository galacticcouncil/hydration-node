function dispatchErrorMessage(api, error) {
  if (!error.isModule) return error.toString();
  const decoded = api.registry.findMetaError(error.asModule);
  return `${decoded.section}.${decoded.name}: ${decoded.docs.join(" ")}`;
}

function submitAndFinalize(api, signer, tx, nonce, timeoutMs) {
  return new Promise((resolve, reject) => {
    let unsubscribe;
    let done = false;
    const finish = (error, result) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      if (unsubscribe) unsubscribe();
      if (error) reject(error);
      else resolve(result);
    };
    const timer = setTimeout(() => finish(new Error(
      `No finalization within ${timeoutMs}ms. Outcome is uncertain; stopping. ` +
      "Check finalized chain state before rerunning."
    )), timeoutMs);

    Promise.resolve().then(() => tx.signAndSend(signer, { nonce }, (receipt) => {
      const status = receipt.status;
      if (status.isInvalid || status.isDropped || status.isUsurped || status.isFinalityTimeout) {
        finish(new Error(`Transaction status: ${status.type}; stopping for a fresh state scan`));
      } else if (status.isFinalized) {
        const failure = receipt.events.find(({ event }) => api.events.system.ExtrinsicFailed.is(event));
        if (failure || receipt.dispatchError) {
          const error = receipt.dispatchError || failure.event.data[0];
          finish(new Error(dispatchErrorMessage(api, error)));
        } else if (!receipt.events.some(({ event }) => api.events.utility.BatchCompleted.is(event))) {
          finish(new Error("Finalized transaction has no utility.BatchCompleted event"));
        } else {
          finish(null, receipt);
        }
      }
    })).then((unsub) => {
      unsubscribe = unsub;
      if (done) unsubscribe();
    }).catch((error) => finish(error));
  });
}

module.exports = { submitAndFinalize };
