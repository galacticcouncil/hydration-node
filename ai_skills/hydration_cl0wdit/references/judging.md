# Finding validation

Apply four gates to each candidate and save its disposition. Evidence can change a verdict;
there is no rule against revisiting an earlier conclusion. Unknown behavior remains unknown.

## 1. Execution

Trace caller or lifecycle trigger through every relevant guard, hook, helper, cross-pallet
call, transaction boundary and mutation to the claimed harm. Quote the exact blocking check
when rejecting a claim. A name, comment, conventional library behavior or speculative guard
is not a refutation; equally, an uncertain path is not an executable path.

## 2. Reachable state

Show how the required state arises at the reviewed revision/configuration, or label the
unverified deployment assumption. For upgrades, include old storage shapes, bounds, versions,
locks, reserves, paused/empty states and partial prior cleanup. Do not require a production
exploit to be run against a live chain to prove a source-level defect.

## 3. Trigger and trust

Identify who controls inputs and who triggers execution. Distinguish a trusted actor abusing
intended authority from a defect during normal authorized operation. A normal upgrade that
traps block production, a keeper action that loses funds, or a hook that corrupts state can
be reportable without an attacker holding governance privileges. State that prerequisite and
avoid describing it as an unprivileged exploit when it is an operational failure.

For economic attacks, calculate realistic capital, fees, repetition and extraction. An attack
with no profit can still be a supported availability/resource-exhaustion finding if its cost
and victim impact are material. Do not reject denial of service solely because it pays no
attacker profit.

## 4. Material impact

Name the victim and supported loss, blocked operation, consensus/upgrade failure, or persistent
resource use. Separate duration, recoverability and scope from severity. Self-harm, ordinary
privilege, version-bump housekeeping and missing tests without a demonstrated defect are not
findings. Compounding dust requires a concrete repetition/cost argument.

## Verdicts, severity and confidence

- `FINDING`: every required step is evidenced, with a supported severity (Critical/High/Medium/
  Low), confidence, path, proof, suggested fix and verification status.
- `LEAD`: a specific unresolved mechanism with locations, uncertainty, a next check and a
  mitigation when supportable. Do not assign a confidence score or confirmed severity.
- `REJECTED`: a concrete counterexample or excluded scope, recorded with candidate ID and reason.

Confidence is evidence strength, not a checklist score and not severity. The executable
threshold is `CONFIDENCE_THRESHOLD` in `scripts/audit_artifacts.py` (75). Below that standard,
keep a lead. Every confirmed finding gets a concrete suggested fix regardless of severity.
A completed trace may promote a lead. Agent convergence, repeated observations, and a similar
bug in another pallet are reasons to investigate, never substitutes for that pallet's proof.

Review fixes for availability, accounting, access and repeated instances. State alternatives
when they are independently safe. Reject an unsafe raw-agent fix rather than preserving it
verbatim as a recommendation.

`known-false-positives.md` provides questions to disprove common claims. None is an automatic
exemption: verify the pinned SDK implementation and runtime wiring. Fixed-point wrappers,
bounded collections, comments and trusted origins do not prove safety by themselves.
