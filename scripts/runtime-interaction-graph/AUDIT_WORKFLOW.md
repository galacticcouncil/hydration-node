# Runtime interaction graph audit workflow

1. Generate the source graph and compiler manifests.
2. Review `coverage.json` and failed semantic-analysis packages before relying on absence of edges.
3. Use `interaction-graph.html` to filter by component, boundary, storage, origin, selector, or provenance.
4. Review `projections-active/` for cycles and cross-domain paths, then inspect callback, state, asset,
	authorization, configuration, EVM-interface, and deployment projections independently. Use `projections/`
	when auditing inventory-only evidence. Configuration or deployment relationships never create execution cycles.
5. Pin EVM and Substrate observations to the same block number, enforce the descriptor's Substrate genesis hash
   and runtime spec name, and review `evm-build-provenance.json` for chain identities, block hashes, sibling
   commits, input hashes, and explicitly supplied semantic manifests.
6. Compare graph and contract snapshots before and after a runtime or deployment change.
7. Convert important expected paths into regression fixtures in `historical-interactions.json`.
8. Review `integration-test-coverage.md` for graph-only components and entrypoints, then inspect tests before
   deciding whether a missing link represents a real coverage gap.
9. Review `completeness.md`, `resolution-coverage.md`, MIR source-function coverage, path-search truncation
	metadata, and `prioritized-test-gaps.md`; full semantic runs enforce 46/46 MIR packages, zero unresolved
	runtime targets, and explicit reasons for inventory-only traits and migrations.

The graph records evidence and ambiguity. A missing edge is meaningful only when the relevant package and
resolution coverage are complete. Cycles, ordering matches, and cross-domain paths are review targets, not
standalone vulnerability findings.
