async function planCleanup(entries, lockedAccounts, referendumInfo, limit) {
  const calls = [];
  const selectedAccounts = new Set();
  const statuses = new Map();
  let selectedVotes = 0;
  let ongoingVotes = 0;
  let deferredVotes = 0;
  let delegatingRecords = 0;

  for (const [key, voting] of entries) {
    const target = key.args[0].toString();
    if (voting.isDelegating) {
      delegatingRecords += 1;
      continue;
    }
    if (!voting.isDirect) throw new Error(`Unknown democracy voting variant for ${target}`);

    for (const [index] of voting.asDirect.votes) {
      const refIndex = index.toNumber();
      if (!statuses.has(refIndex)) {
        const info = await referendumInfo(refIndex);
        statuses.set(refIndex, info.isSome && info.unwrap().isOngoing);
      }
      if (statuses.get(refIndex)) {
        ongoingVotes += 1;
      } else if (limit !== undefined && selectedVotes >= limit) {
        deferredVotes += 1;
      } else {
        calls.push({ kind: "forceRemoveVote", target, refIndex });
        selectedAccounts.add(target);
        selectedVotes += 1;
      }
    }
  }

  // Unlock follows vote removal, including when a target spans multiple batches.
  // Empty Direct records can still carry an expired prior lock from an earlier run.
  for (const [key, voting] of entries) {
    const target = key.args[0].toString();
    if (voting.isDirect && (selectedAccounts.has(target) || lockedAccounts.has(target))) {
      calls.push({ kind: "unlock", target });
    }
  }

  return { calls, selectedVotes, ongoingVotes, deferredVotes, delegatingRecords };
}

function batchCalls(api, calls, size) {
  const batches = [];
  for (let offset = 0; offset < calls.length; offset += size) {
    const chunk = calls.slice(offset, offset + size);
    const txs = chunk.map((call) => call.kind === "unlock"
      ? api.tx.democracy.unlock(call.target)
      : api.tx.democracy.forceRemoveVote(call.target, call.refIndex));
    batches.push({ calls: chunk, tx: api.tx.utility.batchAll(txs) });
  }
  return batches;
}

module.exports = { planCleanup, batchCalls };
