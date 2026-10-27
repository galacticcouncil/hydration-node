const test = require("node:test");
const assert = require("node:assert/strict");
const { planCleanup, batchCalls } = require("./planner");
const { submitAndFinalize } = require("./submit");

const entry = (target, indices = [], delegating = false) => [
  { args: [{ toString: () => target }] },
  { isDirect: !delegating, isDelegating: delegating,
    asDirect: { votes: indices.map((i) => [{ toNumber: () => i }]) } },
];
const finished = async () => ({ isSome: true, unwrap: () => ({ isOngoing: false }) });

test("cleanup_should_select_finished_and_cancelled_votes_when_ongoing_votes_and_delegations_exist", async () => {
  const entries = [entry("alice", [1, 2, 3]), entry("bob", [], true), entry("carol")];
  const statuses = async (i) => ({ isSome: i !== 3, unwrap: () => ({ isOngoing: i === 2 }) });
  const plan = await planCleanup(entries, new Set(["alice", "bob", "carol"]), statuses);
  assert.deepEqual(plan.calls, [
    { kind: "forceRemoveVote", target: "alice", refIndex: 1 },
    { kind: "forceRemoveVote", target: "alice", refIndex: 3 },
    { kind: "unlock", target: "alice" },
    { kind: "unlock", target: "carol" },
  ]);
  assert.equal(plan.ongoingVotes, 1);
  assert.equal(plan.delegatingRecords, 1);
});

test("cleanup_should_preserve_unlock_calls_when_vote_limit_is_reached", async () => {
  const plan = await planCleanup([entry("alice", [1, 2]), entry("bob", [1]), entry("carol")],
    new Set(["carol"]), finished, 1);
  assert.equal(plan.selectedVotes, 1);
  assert.equal(plan.deferredVotes, 2);
  assert.deepEqual(plan.calls.slice(1), [{ kind: "unlock", target: "alice" }, { kind: "unlock", target: "carol" }]);
});

test("cleanup_should_unlock_expired_locks_when_previous_run_removed_votes", async () => {
  const plan = await planCleanup([entry("alice"), entry("bob")], new Set(["alice"]), finished);
  assert.deepEqual(plan.calls, [{ kind: "unlock", target: "alice" }]);
});

test("batches_should_remove_votes_before_unlocking_when_account_spans_batches", async () => {
  const plan = await planCleanup([entry("alice", [1, 2, 3])], new Set(), finished);
  const api = { tx: {
    democracy: { forceRemoveVote: (who, i) => `remove:${who}:${i}`, unlock: (who) => `unlock:${who}` },
    utility: { batchAll: (calls) => calls },
  } };
  assert.deepEqual(batchCalls(api, plan.calls, 2).map((b) => b.tx), [
    ["remove:alice:1", "remove:alice:2"], ["remove:alice:3", "unlock:alice"],
  ]);
});

const api = { events: {
  system: { ExtrinsicFailed: { is: (event) => event.name === "failed" } },
  utility: { BatchCompleted: { is: (event) => event.name === "completed" } },
} };

test("submission_should_wait_for_finalization_when_callback_precedes_subscription", async () => {
  let unsubscribed = false;
  const tx = { signAndSend: async (_signer, _options, callback) => {
    callback({ status: { isInBlock: true }, events: [] });
    callback({ status: { isFinalized: true }, events: [{ event: { name: "completed" } }] });
    return () => { unsubscribed = true; };
  } };
  await submitAndFinalize(api, {}, tx, 0, 1000);
  await new Promise(setImmediate);
  assert.equal(unsubscribed, true);
});

test("submission_should_fail_when_finalized_atomic_batch_fails", async () => {
  const error = { isModule: false, toString: () => "BadOrigin" };
  const tx = { signAndSend: async (_signer, _options, callback) => {
    callback({ status: { isFinalized: true }, events: [{ event: { name: "failed", data: [error] } }] });
    return () => {};
  } };
  await assert.rejects(submitAndFinalize(api, {}, tx, 0, 1000), /BadOrigin/);
});

test("submission_should_stop_and_unsubscribe_when_finalization_times_out", async () => {
  let unsubscribed = false;
  const tx = { signAndSend: async () => () => { unsubscribed = true; } };
  await assert.rejects(submitAndFinalize(api, {}, tx, 0, 10), /Outcome is uncertain/);
  assert.equal(unsubscribed, true);
});
