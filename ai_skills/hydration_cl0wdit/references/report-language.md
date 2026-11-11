# Report language

Use the plain-language principles of Pashov's Simplified Technical English guidance without
compressing away evidence or preconditions.

- Give titles a concrete defect or effect. Name the actor, trigger, affected state and harm in
  the description. Prefer active voice and short sentences, usually at most 25 words each.
- State event order in causal order. Separate the defect, required state, and impact when one
  sentence would obscure them. Do not claim certainty when a path remains unverified.
- Avoid metaphors, inflated severity adjectives, unexplained jargon and vague "can be abused"
  claims. Explain the amount, duration or scope of harm where it is known.
- Preserve code, identifiers, paths, numeric values, bug-class labels, candidate IDs and model
  metadata exactly. They are evidence or matching keys, not prose to simplify.
- Treat confidence and severity as separate fields. Repeated passes or agreement among agents
  are provenance, not a reason to increase severity or promote an incomplete trace.
- A lead names the missing check or evidence and the next action needed to resolve it. A
  concise description does not replace the full trace, proof and suggested fix.
