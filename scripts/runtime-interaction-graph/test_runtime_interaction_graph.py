import importlib.util
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path
from unittest import mock


MODULE = Path(__file__).with_name("runtime_interaction_graph.py")
SPEC = importlib.util.spec_from_file_location("runtime_interaction_graph", MODULE)
graph = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(graph)
DIFF_SPEC = importlib.util.spec_from_file_location("diff_snapshots", MODULE.with_name("diff_snapshots.py"))
snapshot_diff = importlib.util.module_from_spec(DIFF_SPEC)
DIFF_SPEC.loader.exec_module(snapshot_diff)
GRAPH_DIFF_SPEC = importlib.util.spec_from_file_location("diff_graphs", MODULE.with_name("diff_graphs.py"))
graph_diff = importlib.util.module_from_spec(GRAPH_DIFF_SPEC)
GRAPH_DIFF_SPEC.loader.exec_module(graph_diff)


def semantic_provenance(tool: str, source_inputs: dict | None = None) -> dict:
	collector_name = "collect_mir.py" if tool == "rustc-mir" else "collect_rapx.py"
	tool_inputs = graph.semantic_tool_inputs(tool)
	toolchain = graph.collect_mir.TOOLCHAIN if tool == "rustc-mir" else graph.collect_rapx.TOOLCHAIN
	return {"source_inputs": source_inputs or {"sha256": "0" * 64, "file_count": 1},
		"collector_sha256": tool_inputs["files"][collector_name], "tool_inputs": tool_inputs,
		"toolchain": toolchain}


class RuntimeInteractionGraphTests(unittest.TestCase):
	def test_body_end_handles_nested_blocks(self):
		text = "fn f() { if true { call(); } storage(); } fn g() {}"
		self.assertEqual(text[: graph.body_end(text, 0)], "fn f() { if true { call(); } storage(); }")

	def test_function_body_end_rejects_bodyless_trait_declarations(self):
		text = "trait T { fn declared(value: [u8; 32]); fn implemented() { call(); } }"
		declared, implemented = list(graph.FN.finditer(text))
		self.assertIsNone(graph.function_body_end(text, declared.end()))
		self.assertEqual(text[implemented.start():graph.function_body_end(text, implemented.end())],
			"fn implemented() { call(); }")

	def test_source_ids_do_not_change_with_line_offsets(self):
		self.assertEqual(graph.source_id("function", "pallets/example/src/lib.rs", "call", 1),
			"function:pallets/example/src/lib.rs:call")
		self.assertEqual(graph.source_id("function", "pallets/example/src/lib.rs", "call", 2),
			"function:pallets/example/src/lib.rs:call:2")

	def test_runtime_aliases_resolve_construct_runtime_entries(self):
		text = """
 Router: pallet_route_executor = 67,
 Stableswap:
	 pallet_stableswap
	 exclude_parts { GenesisConfig }
	 = 70,
 Ethereum: pallet_ethereum = 92,
 Ethereum::on_finalize(System::block_number() + 1);
 if matches!(version, 3..=5) {}
"""
		self.assertEqual(
			graph.runtime_aliases(text),
			{"Router": "pallet_route_executor", "Stableswap": "pallet_stableswap",
				"Ethereum": "pallet_ethereum"},
		)

	def test_runtime_config_blocks_keep_adjacent_implementations_separate(self):
		text = """impl pallet_first::Config for Runtime {
	type Handler = FirstHandler;
}
impl pallet_second::Config for Runtime {
	type Handler = SecondHandler;
}
impl pallet_last::Config for Runtime {
	type Handler = LastHandler;
}"""
		blocks = {match.group(1): body for match, body in graph.runtime_config_blocks(text)}
		self.assertEqual(set(blocks), {"pallet_first", "pallet_second", "pallet_last"})
		self.assertIn("FirstHandler", blocks["pallet_first"])
		self.assertNotIn("SecondHandler", blocks["pallet_first"])
		self.assertIn("SecondHandler", blocks["pallet_second"])
		self.assertNotIn("LastHandler", blocks["pallet_second"])
		self.assertIn("LastHandler", blocks["pallet_last"])

	def test_config_callback_targets_should_ignore_nested_generic_types(self):
		value = """WrapRunner<
			Self,
			pallet_evm::runner::stack::Runner<Self>,
			hydradx_adapters::price::FeeAssetBalanceInCurrency<
				Runtime,
				ConvertBalance<TenMinutesOraclePrice, XykPaymentAssetSupport, DotAssetId>,
				FeeCurrencyOverrideOrDefault,
				FungibleCurrencies<Runtime>,
			>,
		>"""
		local_symbols = {
			"WrapRunner": {"component:evm:runner"},
			"FeeAssetBalanceInCurrency": {"component:runtime-adapters:price"},
			"ConvertBalance": {"component:runtime-adapters:price"},
			"FeeCurrencyOverrideOrDefault": {"component:runtime:assets"},
			"FungibleCurrencies": {"component:runtime:assets"},
		}
		self.assertEqual(graph.config_callback_targets(value, {}, local_symbols),
			["component:evm:runner"])

	def test_config_callback_targets_should_keep_top_level_tuple_members(self):
		aliases = {
			"Omnipool": "pallet_omnipool",
			"Stableswap": "pallet_stableswap",
			"XYK": "pallet_xyk",
			"LBP": "pallet_lbp",
			"HSM": "pallet_hsm",
		}
		self.assertEqual(
			graph.config_callback_targets(
				"(Omnipool, Stableswap, XYK, LBP, Aave, HSM)", aliases,
				{"Aave": {"component:evm:aave_trade_executor"}},
			),
			[
				"component:evm:aave_trade_executor", "pallet:hsm", "pallet:lbp", "pallet:omnipool",
				"pallet:stableswap", "pallet:xyk",
			],
		)

	def test_component_id_should_canonicalize_warehouse_liquidity_mining(self):
		self.assertEqual(graph.component_id("warehouse_liquidity_mining"), "pallet:liquidity-mining")
		self.assertEqual(graph.component_id("pallet_liquidity_mining"), "pallet:liquidity-mining")

	def test_graph_edges_are_exactly_deduplicated_and_have_nodes(self):
		g = graph.Graph()
		g.edge("source", "target", "calls", evidence=["same"])
		g.edge("source", "target", "calls", evidence=["same"])
		g.edge("source", "target", "calls", evidence=["different"])
		self.assertEqual(len(g.edges), 2)
		self.assertEqual(set(g.nodes), {"source", "target"})
		self.assertTrue(g.nodes["source"]["placeholder"])
		g.node("source", "function", name="source")
		self.assertEqual(g.nodes["source"]["kind"], "function")
		self.assertNotIn("placeholder", g.nodes["source"])

	def test_semantic_inventory_should_be_loaded_only_from_scanned_root(self):
		with tempfile.TemporaryDirectory() as directory:
			base = Path(directory) / "base"
			head = Path(directory) / "head"
			base.mkdir()
			(head / "scripts/runtime-interaction-graph").mkdir(parents=True)
			(head / "pallets/example/src").mkdir(parents=True)
			(head / "pallets/example/src/lib.rs").write_text("pub struct ExampleLedger;\n")
			inventory = {
				"schema_version": 1,
				"nodes": [{
					"id": "ledger:example",
					"kind": "ledger",
					"domain": "asset-routing",
					"label": "Example ledger",
					"description": "Inventory local to the scanned head root.",
					"evidence": [{"file": "pallets/example/src/lib.rs", "symbol": "pub struct ExampleLedger;"}],
				}],
				"edges": [],
			}
			(head / "scripts/runtime-interaction-graph/semantic-inventory.json").write_text(
				graph.json.dumps(inventory))

			base_graph = graph.Graph()
			head_graph = graph.Graph()
			self.assertIsNone(graph.merge_semantic_inventory(base_graph, base))
			self.assertNotIn("semantic-analysis:explicit-inventory", base_graph.nodes)
			self.assertIsNotNone(graph.merge_semantic_inventory(head_graph, head))
			self.assertIn("ledger:example", head_graph.nodes)
			self.assertEqual(head_graph.nodes["semantic-analysis:explicit-inventory"]["node_count"], 1)

	def test_generated_and_helper_functions_are_not_entrypoint_eligible(self):
		text = '#[cfg(feature = "runtime-benchmarks")]\npub mod benchmark_helpers { fn call() {} }\nfn live() {}'
		ranges = graph.helper_module_ranges(text)
		helper = next(match for match in graph.FN.finditer(text) if match.group(1) == "call")
		live = next(match for match in graph.FN.finditer(text) if match.group(1) == "live")
		self.assertFalse(graph.entrypoint_eligible(Path("runtime/helpers.rs"), helper.start(), ranges))
		self.assertTrue(graph.entrypoint_eligible(Path("runtime/helpers.rs"), live.start(), ranges))
		self.assertFalse(graph.entrypoint_eligible(Path("pallets/example/src/weights.rs"), live.start(), []))
		self.assertTrue(graph.source_excluded(Path("precompiles/utils/src/testing/execution.rs")))
		self.assertTrue(graph.source_excluded(
			Path("runtime/hydradx/src/evm/evm-utility/macro/src/lib.rs")))

	def test_inactive_cfg_ranges_should_exclude_benchmarks_but_not_production_fallbacks(self):
		text = '''
#[cfg(feature = "runtime-benchmarks")]
impl Benchmark for Runtime { fn dispatch_benchmark() {} }
#[cfg(not(feature = "runtime-benchmarks"))]
impl Live for Runtime { fn execute() {} }
'''
		ranges = graph.inactive_cfg_ranges(text)
		benchmark = text.index("dispatch_benchmark")
		live = text.index("fn execute")
		self.assertTrue(any(start <= benchmark < end for start, end in ranges))
		self.assertFalse(any(start <= live < end for start, end in ranges))

	def test_config_associated_types_should_exclude_inactive_cfg_declarations(self):
		text = '''
pub trait Config {
	type Live: Live;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper: Helper;
	#[cfg(not(feature = "runtime-benchmarks"))]
	type ProductionHelper: Helper;
}
'''
		self.assertEqual(graph.config_associated_types(text), {
			"Live": "Live",
			"ProductionHelper": "Helper",
		})

	def test_config_associated_types_should_parse_gats_and_ignore_documentation_examples(self):
		text = '''
//! pub trait Config { type Fake: Fake; }
pub trait Config {
	type TryCallCurrency<'a>: TryConvert<&'a <Self as frame_system::Config>::RuntimeCall, AssetIdOf<Self>>;
}
'''
		self.assertEqual(graph.config_associated_types(text), {
			"TryCallCurrency": "TryConvert<&'a <Self as frame_system::Config>::RuntimeCall, AssetIdOf<Self>>",
		})

	def test_config_type_items_should_parse_gat_assignments(self):
		items = graph.config_type_items("type TryCallCurrency<'a> = TryCallCurrency;", "=")
		self.assertEqual([(item["name"], item["generics"], item["value"]) for item in items],
			[("TryCallCurrency", "<'a>", "TryCallCurrency")])

	def test_empty_root_config_should_anchor_nested_source_modules(self):
		declarations = {"pallet_example::Config": {}}
		self.assertEqual(
			graph.nearest_config_trait("pallet_example::types::Config", declarations),
			"pallet_example::Config",
		)

	def test_rust_use_bindings_should_expand_groups_and_canonicalize_config_aliases(self):
		text = '''
use pallet_evm::{self, runner::Runner as EvmRunnerT, Config};
pub(crate) use pallet_collective as pallet_collective_technical_committee;
'''
		bindings = graph.rust_use_bindings(text)
		self.assertEqual(bindings["Config"], {"pallet_evm::Config"})
		self.assertEqual(bindings["EvmRunnerT"], {"pallet_evm::runner::Runner"})
		self.assertEqual(graph.config_reference("Config", bindings, None),
			("pallet_evm::Config", None))
		self.assertEqual(graph.config_reference(
			"pallet_collective_technical_committee::Config<TechnicalCollective>", bindings, None,
			{"TechnicalCollective": {"pallet_collective::Instance2"}}),
			("pallet_collective::Config", "Instance2"))
		local = graph.rust_use_bindings("use crate::{Config, Pallet};")
		self.assertEqual(graph.config_reference("Config", local, "pallet_currencies::Config"),
			("pallet_currencies::Config", None))
		aliased_self = graph.rust_use_bindings(
			"use cumulus_pallet_parachain_system::{self as parachain_system, Config};")
		self.assertEqual(aliased_self["parachain_system"], {"cumulus_pallet_parachain_system"})
		self.assertEqual(graph.config_reference("parachain_system::Config", aliased_self, None),
			("cumulus_pallet_parachain_system::Config", None))
		self.assertEqual(graph.config_reference("super::pallet::Config", {}, "pallet_aura_ext::Config"),
			("pallet_aura_ext::Config", None))

	def test_associated_calls_should_preserve_explicit_trait_qualification(self):
		calls = graph.associated_calls('''
<T as pallet_evm::Config>::GasWeightMapping::gas_to_weight(1, true);
T::AddressMapping::into_account_id(address);
<Runtime as pallet_evm::Config>::ChainId::get();
<R as pallet_evm::Config>::PrecompilesValue::get();
R::AddressMapping::into_account_id(address);
''')
		self.assertEqual([(call["subject"], call["trait_path"], call["associated_type"], call["method"])
			for call in calls], [
			("T", "pallet_evm::Config", "GasWeightMapping", "gas_to_weight"),
			("T", None, "AddressMapping", "into_account_id"),
			("Runtime", "pallet_evm::Config", "ChainId", "get"),
			("R", "pallet_evm::Config", "PrecompilesValue", "get"),
			("R", None, "AddressMapping", "into_account_id"),
		])

	def test_rust_mask_should_preserve_lifetimes_and_ignore_literal_or_comment_calls(self):
		text = '''
fn example<'a>(value: &'a str) {
	let _ = r#"T::LiteralHandler::call(); // not a comment"#;
	let _ = 'x';
	// T::CommentHandler::call();
	T::LiveHandler::call();
}
'''
		calls = graph.associated_calls(graph.mask_rust_comments(text))
		self.assertEqual([(call["associated_type"], call["method"]) for call in calls],
			[("LiveHandler", "call")])

	def test_mir_associated_call_should_require_config_projection_as_receiver(self):
		self.assertEqual(graph.mir_associated_call(
			"<<T as pallet::Config<I>>::MultiCurrency as orml_traits::MultiCurrency<A>>::transfer"), {
			"subject": "T", "trait_path": "pallet::Config",
			"config_instance": "I",
			"associated_type": "MultiCurrency", "method": "transfer",
		})
		self.assertEqual(graph.mir_associated_call(
			"<<R as pallet_evm::Config>::AddressMapping as AddressMapping<A>>::into_account_id"), {
			"subject": "R", "trait_path": "pallet_evm::Config",
			"config_instance": None,
			"associated_type": "AddressMapping", "method": "into_account_id",
		})
		self.assertIsNone(graph.mir_associated_call(
			"<<<R as pallet_evm::Config>::Timestamp as Time>::Moment as Convert<u128>>::convert"))
		self.assertIsNone(graph.mir_associated_call(
			"<Result<<T as pallet::Config>::Handler, E> as Try>::branch"))

	def test_associated_config_references_should_stop_at_nearest_redeclaration(self):
		declarations = {
			"pallet_child::Config": {"Currency": "Currency"},
			"pallet_parent::Config": {"Currency": "Currency"},
		}
		parents = {"pallet_child::Config": {"pallet_parent::Config"}}
		self.assertEqual(graph.associated_config_references(
			{("pallet_child::Config", None)}, "Currency", declarations, parents),
			{("pallet_child::Config", None)},
		)

	def test_outputs_are_deterministic(self):
		g = graph.Graph()
		g.node("b", "pallet")
		g.node("a", "runtime")
		g.edge("a", "b", "contains")
		with tempfile.TemporaryDirectory() as directory:
			out = Path(directory)
			graph.write_outputs(g, out)
			first = (out / "interaction-graph.json").read_text()
			graph.write_outputs(g, out)
			self.assertEqual(first, (out / "interaction-graph.json").read_text())

	def test_graph_scale_summary_should_separate_activity_and_evidence_counts(self):
		g = graph.Graph()
		g.node("pallet:active", "pallet", domain="frame", runtime_active=True)
		g.node("component:inactive", "evm-adapter", domain="evm-adapter", runtime_active=False)
		g.node("entrypoint:active", "entrypoint", domain="frame", runtime_active=True)
		g.node("associated:inactive:Handler", "associated-type", owner="component:inactive", runtime_active=False)
		g.node("mir-operation:active", "mir-operation", function="entrypoint:active")
		g.node("contract-function:foo", "contract-function")
		g.edge("pallet:active", "entrypoint:active", "direct-call")
		g.edge("pallet:active", "entrypoint:active", "mir-call", semantic_source="rustc-mir")
		g.edge("pallet:active", "component:inactive", "rapx-call", semantic_source="rapx")
		g.edge("pallet:active", "entrypoint:active", "enforces", semantic_source="explicit-inventory")
		g.edge("pallet:active", "entrypoint:active", "selector-matches-contract-function")
		g.edge("pallet:active", "contract-function:foo", "exposes-function")

		summary = graph.graph_scale_summary(g)
		self.assertEqual(summary["totals"], {
			"nodes": {"raw": 6, "operational": 4, "active": 2, "unclassified": 2, "inactive": 2},
			"edges": {"raw": 6, "operational": 5, "active": 4, "unclassified": 1, "inactive": 1},
			"components": {"raw": 2, "operational": 1, "active": 1, "unclassified": 0, "inactive": 1},
			"entrypoints": {"raw": 1, "operational": 1, "active": 1, "unclassified": 0, "inactive": 0},
		})
		self.assertEqual({item["name"]: (item["raw"], item["operational"], item["active"],
			item["unclassified"], item["inactive"]) for item in summary["domains"]}, {
			"frame": (3, 3, 2, 1, 0),
			"evm-adapter": (2, 0, 0, 0, 2),
			"unclassified": (1, 1, 0, 1, 0),
		})
		self.assertEqual(summary["inventory_only_targets"], 1)
		self.assertEqual(summary["unresolved_targets"], 0)
		self.assertEqual(summary["evidence"], [
			{"name": "source scan", "raw": 1, "operational": 1,
				"active": 1, "unclassified": 0, "inactive": 0},
			{"name": "rustc MIR", "raw": 1, "operational": 1,
				"active": 1, "unclassified": 0, "inactive": 0},
			{"name": "RAPx", "raw": 1, "operational": 0,
				"active": 0, "unclassified": 0, "inactive": 1},
			{"name": "semantic inventory", "raw": 1, "operational": 1,
				"active": 1, "unclassified": 0, "inactive": 0},
			{"name": "deployment / chain", "raw": 2, "operational": 2,
				"active": 1, "unclassified": 1, "inactive": 0},
		])

	def test_graph_scale_svg_should_be_bounded_and_deterministic(self):
		g = graph.Graph()
		g.node("pallet:active", "pallet", domain="frame")
		g.node("boundary:evm", "execution-boundary", domain="evm")
		g.edge("pallet:active", "boundary:evm", "enters-evm")
		with tempfile.TemporaryDirectory() as directory:
			output = Path(directory)
			path = graph.write_graph_scale_svg(g, output)
			first = path.read_bytes()
			root = ET.fromstring(first)
			self.assertEqual((root.attrib["width"], root.attrib["height"]), ("1600", "1200"))
			self.assertEqual(root.attrib["aria-labelledby"], "title")
			self.assertEqual(root.attrib["aria-describedby"], "description")
			self.assertIn("Hydration runtime interaction graph", first.decode())
			self.assertIn("unclassified", first.decode())
			self.assertIn("fill:#475569", first.decode())
			graph.write_graph_scale_svg(g, output)
			self.assertEqual(first, path.read_bytes())

	def test_component_projection_should_preserve_edge_evidence(self):
		g = graph.Graph()
		g.node("function:a", "function", owner="pallet:a")
		g.node("function:b", "function", owner="pallet:b")
		g.edge("function:a", "function:b", "direct-call", file="pallets/a/src/lib.rs", line=42,
			selector="0x12345678", storage="Pools", block_hash="0x" + "a" * 64,
			semantic_source="source")
		edge = graph.component_edges(g)[0]
		self.assertEqual((edge["source"], edge["target"]), ("pallet:a", "pallet:b"))
		self.assertEqual(edge["evidence_source"], "function:a")
		self.assertEqual(edge["evidence_target"], "function:b")
		self.assertEqual(edge["selector"], "0x12345678")
		self.assertEqual(edge["storage"], "Pools")
		self.assertEqual(edge["block_hash"], "0x" + "a" * 64)

	def test_interactive_html_inline_scripts_should_compile(self):
		g = graph.Graph()
		g.node("pallet:a", "pallet", artifact="<untrusted>&evidence")
		g.node("pallet:b", "pallet")
		g.edge("pallet:a", "pallet:b", "contains", selector="0x12345678")
		with tempfile.TemporaryDirectory() as directory:
			output = Path(directory)
			graph.write_interactive_html(g, graph.component_edges(g), output)
			contents = (output / "interaction-graph.html").read_text()
			self.assertIn("&evidence", contents)
			self.assertIn('href="graph-scale.svg"', contents)
			for script in graph.re.findall(r"<script>(.*?)</script>", contents, graph.re.DOTALL):
				try:
					completed = graph.subprocess.run(["node", "--check"], input=script,
						text=True, capture_output=True)
				except FileNotFoundError:
					self.skipTest("node is unavailable")
				self.assertEqual(completed.returncode, 0, completed.stderr)

	def test_strongly_connected_reports_cycles_only(self):
		edges = [
			{"source": "a", "target": "b"},
			{"source": "b", "target": "a"},
			{"source": "b", "target": "c"},
		]
		self.assertEqual(graph.strongly_connected(edges), [["a", "b"]])

	def test_storage_reads_are_not_mutations(self):
		self.assertNotIn("get", graph.STORAGE_WRITES)
		self.assertIn("try_mutate", graph.STORAGE_WRITES)
		plain = graph.STORAGE.search("NoncesStorage::insert(key, value)")
		generic = graph.STORAGE.search("Positions::<T>::get(id)")
		associated = graph.STORAGE.search("Currency::get(id)")
		self.assertEqual(plain.groups(), ("NoncesStorage", "insert"))
		self.assertEqual(generic.groups(), ("Positions", "get"))
		self.assertTrue(graph.is_storage_match(plain))
		self.assertTrue(graph.is_storage_match(generic))
		self.assertFalse(graph.is_storage_match(associated))

	def test_evm_and_frame_boundary_patterns(self):
		self.assertTrue(graph.EVM_ENTRY.search("T::Runner::call(source, target)"))
		self.assertTrue(graph.EVM_ENTRY.search("handle.call(address, input)"))
		self.assertEqual(graph.INTERNAL_EVM_EXECUTOR.search("Executor::<Runtime>::call(context)").group(1),
			"call")

	def test_generated_selector_bindings_should_resolve_annotated_import_alias_only(self):
		definition_text = """
#[module_evm_utility_macro::generate_function_selector]
#[repr(u32)]
pub enum WireFunction {
	Supply = "supply(address,uint256)",
}
pub enum ArbitraryFunction {
	Supply = "wrong()",
}
"""
		definitions = {
			("runtime/hydradx", ("evm", "selectors"), "WireFunction"):
				graph.generated_selector_enums(definition_text)["WireFunction"],
		}
		consumer = "use crate::evm::selectors::WireFunction as ImportedFunction;"
		bindings = graph.selector_type_bindings(consumer, "runtime/hydradx/src/caller.rs", definitions)
		self.assertEqual(bindings, {"ImportedFunction": {"Supply": "supply(address,uint256)"}})
		self.assertNotIn("ArbitraryFunction", bindings)

	def test_precompile_public_aliases_should_create_distinct_selector_entrypoints(self):
		text = """
#[precompile::public("freezeAsset()")]
#[precompile::public("freeze_asset()")]
fn freeze() {}
"""
		targets = graph.attribute_target_lists(text, graph.PRECOMPILE_PUBLIC)
		function = next(graph.FN.finditer(text))
		self.assertEqual([match.group(1) for match in targets[function.start()]],
			["freezeAsset()", "freeze_asset()"])
		g = graph.Graph()
		g.node("function:freeze", "function", owner="precompile:test")
		entrypoints = graph.add_precompile_selector_entrypoints(g, "function:freeze",
			[(match.group(1), 1) for match in targets[function.start()]], "precompiles/test/src/lib.rs")
		self.assertEqual(len(entrypoints), 2)
		self.assertEqual(len({g.nodes[entrypoint]["selector"] for entrypoint in entrypoints}), 2)
		self.assertTrue(all(g.nodes[entrypoint]["signature"] in {"freezeAsset()", "freeze_asset()"}
			for entrypoint in entrypoints))
		self.assertTrue(graph.FRAME_DISPATCH.search("RuntimeCall::dispatch(origin)"))

	def test_runtime_binding_should_resolve_dynamic_call(self):
		g = graph.Graph()
		g.node("associated:pallet:source:Router", "associated-type", unresolved=True)
		g.node("function:source", "function", owner="pallet:source")
		g.edge("function:source", "associated:pallet:source:Router", "dynamic-call", method="sell")
		g.edge("associated:pallet:source:Router", "pallet:router", "binding-resolves-to")
		graph.enrich_resolutions(g)
		self.assertFalse(g.nodes["associated:pallet:source:Router"]["unresolved"])
		self.assertTrue(any(edge["kind"] == "resolved-call" and edge["target"] == "pallet:router"
			for edge in g.edges))

	def test_bounded_paths_should_include_resolved_multi_hop_evm_path(self):
		edges = [
			{"source": "pallet:router", "target": "pallet:hsm"},
			{"source": "pallet:hsm", "target": "boundary:evm-execution"},
		]
		self.assertIn(
			["pallet:router", "pallet:hsm", "boundary:evm-execution"],
			graph.bounded_paths(edges, {"boundary:evm-execution"}),
		)

	def test_query_packs_should_bound_traces_per_entrypoint(self):
		g = graph.Graph()
		for start in ("a", "z"):
			entrypoint = f"entrypoint:{start}"
			g.node(entrypoint, "entrypoint")
			for index in range(11):
				boundary = f"boundary:{start}:{index:02}"
				g.node(boundary, "execution-boundary")
				g.edge(entrypoint, boundary, "enters-function")
		projections = {name: [] for name in graph.EDGE_PROJECTIONS}
		projections["execution"] = g.edges
		result = graph.query_packs(g, [], projections, [], [], {}, [])
		self.assertEqual(result["schema_version"], 1)
		traces = result["entrypoint_execution_traces"]
		self.assertEqual(sum(trace[0] == "entrypoint:a" for trace in traces), 10)
		self.assertEqual(sum(trace[0] == "entrypoint:z" for trace in traces), 10)
		self.assertEqual(result["entrypoint_trace_search"]["limit_truncated_entrypoints"],
			["entrypoint:a", "entrypoint:z"])

	def test_query_packs_should_exclude_inventory_only_edges_and_entrypoints(self):
		g = graph.Graph()
		g.node("function:active", "function")
		g.node("function:inactive", "function", runtime_active=False)
		g.node("entrypoint:active", "entrypoint", entrypoint_kind="runtime-hook", function="function:active")
		g.node("entrypoint:inactive", "entrypoint", entrypoint_kind="runtime-hook",
			function="function:inactive")
		g.node("origin:root", "origin")
		g.node("asset-operation:transfer", "asset-operation")
		g.edge("origin:root", "function:active", "authorizes-entry")
		g.edge("origin:root", "function:inactive", "authorizes-entry")
		g.edge("function:active", "asset-operation:transfer", "asset-operation")
		g.edge("function:inactive", "asset-operation:transfer", "asset-operation")
		projections = {name: [] for name in graph.EDGE_PROJECTIONS}
		projections["authorization"] = graph.projected_edges(g, "authorization")
		projections["asset"] = graph.projected_edges(g, "asset")
		result = graph.query_packs(g, [], projections, [], [], {}, [])
		self.assertEqual([edge["target"] for edge in result["privileged_entries"]], ["function:active"])
		self.assertEqual([edge["source"] for edge in result["token_backend_edges"]], ["function:active"])
		self.assertEqual([node["id"] for node in result["lifecycle_entrypoints"]], ["entrypoint:active"])

	def test_configured_migration_should_activate_source_local_helpers(self):
		g = graph.Graph()
		migration_file = "external/pallet_example/migration.rs"
		g.node("function:migration", "function", name="on_runtime_upgrade", file=migration_file,
			local_pallet_calls=[{"method": "migrate_data", "line": 10}])
		g.node("function:helper", "function", name="migrate_data", file=migration_file,
			local_pallet_calls=[])
		g.node("function:unused", "function", name="unused", file=migration_file,
			local_pallet_calls=[])
		g.node("entrypoint:migration", "entrypoint", entrypoint_kind="runtime-migration")
		g.edge("entrypoint:migration", "function:migration", "enters-function")
		graph.enrich_migration_calls(g)
		graph.classify_runtime_activity(g, set(), {}, True)
		self.assertTrue(g.nodes["function:migration"]["runtime_active"])
		self.assertTrue(g.nodes["function:helper"]["runtime_active"])
		self.assertFalse(g.nodes["function:unused"]["runtime_active"])
		self.assertTrue(any(edge["source"] == "function:migration" and edge["target"] == "function:helper"
			and edge.get("resolution") == "source-local-migration" for edge in g.edges))

	def test_rapx_output_should_merge_internal_and_evm_calls(self):
		g = graph.Graph()
		g.node("function:entry", "function", name="entry", owner="pallet:hsm")
		g.node("function:helper", "function", name="helper", owner="pallet:hsm")
		output = """  <impl pallet::Pallet<T>>::entry calls:
    -> <impl pallet::Pallet<T>>::helper
    -> hydradx_traits::evm::EVM::call
"""
		with tempfile.TemporaryDirectory() as directory:
			path = Path(directory) / "rapx.txt"
			path.write_text(output)
			self.assertEqual(graph.merge_rapx(g, path, "pallet:hsm"), 2)
			self.assertTrue(any(edge["target"] == "function:helper" for edge in g.edges))
			self.assertTrue(any(edge["target"] == "boundary:evm-execution" for edge in g.edges))
			self.assertEqual({edge["rapx_output"] for edge in g.edges}, {"rapx.txt"})

	def test_runtime_rapx_should_use_module_identity_for_same_named_functions(self):
		g = graph.Graph()
		root_call = "function:runtime/hydradx/src/lib.rs:call"
		executor_call = "function:runtime/hydradx/src/evm/executor.rs:call"
		g.node(root_call, "function", name="call", owner="runtime:hydradx",
			file="runtime/hydradx/src/lib.rs")
		g.node(executor_call, "function", name="call", owner="component:evm:executor",
			file="runtime/hydradx/src/evm/executor.rs")
		output = """  <evm::executor::Executor<T> as EVM<Result>>::call calls:
    -> pallet_dispatcher::Pallet::<T>::extra_gas
"""
		with tempfile.TemporaryDirectory() as directory:
			path = Path(directory) / "runtime.callgraph.txt"
			path.write_text(output)
			self.assertEqual(graph.merge_rapx(g, path, "runtime:hydradx"), 1)
		edge = next(edge for edge in g.edges if edge["kind"] == "rapx-call")
		self.assertEqual(edge["source"], executor_call)
		self.assertEqual(edge["target"], "pallet:dispatcher")

	def test_rapx_manifest_should_merge_successful_packages_only(self):
		g = graph.Graph()
		g.node("function:entry", "function", name="entry", owner="pallet:hsm")
		with tempfile.TemporaryDirectory() as directory:
			root = Path(directory)
			artifact = root / "hsm.callgraph.txt"
			artifact.write_text(
				"  <impl pallet::Pallet<T>>::entry calls:\n    -> hydradx_traits::evm::EVM::call\n")
			package = {"package": "pallet-hsm", "owner": "pallet:hsm",
				"manifest": "pallets/hsm/Cargo.toml"}
			provenance = semantic_provenance("rapx")
			command = graph.collect_rapx.analysis_command(package, "callgraph", 300)
			fingerprint = graph.analysis_provenance.command_fingerprint(
				provenance, {**package, "analysis": "callgraph"}, command)
			manifest = {"schema_version": 2, "tool": "rapx", "toolchain": graph.collect_rapx.TOOLCHAIN,
				"timeout_seconds": 300, "requested_packages": ["pallet-hsm"],
				"requested_analyses": ["callgraph"],
				"provenance": provenance,
				"packages": [{**package, "analyses": {
					"callgraph": {"status": "ok", "path": artifact.name,
						"command": command, "input_fingerprint": fingerprint,
						"artifact_sha256": graph.hashlib.sha256(artifact.read_bytes()).hexdigest()}}}]}
			(root / "manifest.json").write_text(graph.json.dumps(manifest))
			self.assertEqual(graph.merge_rapx_manifest(g, root / "manifest.json"), 1)

	def test_semantic_manifest_should_fail_when_source_fingerprint_is_stale(self):
		with tempfile.TemporaryDirectory() as directory:
			root = Path(directory)
			cargo = root / "Cargo.toml"
			cargo.write_text("[workspace]\n")
			manifest = {"schema_version": 2, "tool": "rustc-mir",
				"toolchain": graph.collect_mir.TOOLCHAIN, "timeout_seconds": 900,
				"requested_packages": [], "packages": [],
				"provenance": semantic_provenance(
					"rustc-mir", graph.analysis_provenance.tree_fingerprint(root))}
			path = root / "manifest.json"
			graph.validate_semantic_manifest(manifest, path, "rustc-mir", root)
			cargo.write_text("[workspace]\nmembers = []\n")
			with self.assertRaisesRegex(ValueError, "source fingerprint is stale"):
				graph.validate_semantic_manifest(manifest, path, "rustc-mir", root)

	def test_semantic_manifest_should_reject_escaping_artifact_paths(self):
		package = {"package": "p", "owner": "pallet:p", "manifest": "pallets/p/Cargo.toml"}
		provenance = semantic_provenance("rustc-mir")
		command = graph.collect_mir.mir_command(package)
		manifest = {"schema_version": 2, "tool": "rustc-mir",
			"toolchain": graph.collect_mir.TOOLCHAIN, "timeout_seconds": 900,
			"requested_packages": ["p"], "provenance": provenance,
			"packages": [{**package, "status": "ok", "artifact": "../outside.mir",
				"command": command, "input_fingerprint": graph.analysis_provenance.command_fingerprint(
					provenance, package, command), "artifact_sha256": "3" * 64}]}
		with tempfile.TemporaryDirectory() as directory:
			path = Path(directory) / "manifest.json"
			with self.assertRaisesRegex(ValueError, "stay inside"):
				graph.validate_semantic_manifest(manifest, path, "rustc-mir")

	def test_rapx_manifest_should_require_every_requested_analysis(self):
		manifest = {"schema_version": 2, "tool": "rapx", "toolchain": graph.collect_rapx.TOOLCHAIN,
			"timeout_seconds": 300, "requested_packages": ["p"],
			"requested_analyses": ["callgraph", "dataflow"],
			"provenance": semantic_provenance("rapx"),
			"packages": [{"package": "p", "owner": "pallet:p", "manifest": "pallets/p/Cargo.toml",
				"analyses": {"callgraph": {"status": "timeout", "path": "p.txt",
					"input_fingerprint": "2" * 64}}}]}
		with tempfile.TemporaryDirectory() as directory:
			with self.assertRaisesRegex(ValueError, "analysis inventory is incomplete"):
				graph.validate_semantic_manifest(manifest, Path(directory) / "manifest.json", "rapx")

	def test_semantic_manifest_should_bind_artifact_to_exact_command(self):
		package = {"package": "p", "owner": "pallet:p", "manifest": "pallets/p/Cargo.toml"}
		provenance = semantic_provenance("rustc-mir")
		command = graph.collect_mir.mir_command(package)
		manifest = {"schema_version": 2, "tool": "rustc-mir",
			"toolchain": graph.collect_mir.TOOLCHAIN, "timeout_seconds": 900,
			"requested_packages": ["p"], "provenance": provenance,
			"packages": [{**package, "status": "timeout", "artifact": "p.mir",
				"command": [*command, "--tampered"],
				"input_fingerprint": graph.analysis_provenance.command_fingerprint(
					provenance, package, command)}]}
		with tempfile.TemporaryDirectory() as directory:
			with self.assertRaisesRegex(ValueError, "artifact command is invalid"):
				graph.validate_semantic_manifest(manifest, Path(directory) / "manifest.json", "rustc-mir")

	def test_semantic_manifest_should_reject_tampered_tool_input_aggregate(self):
		provenance = semantic_provenance("rustc-mir")
		provenance["tool_inputs"] = {**provenance["tool_inputs"],
			"files": {**provenance["tool_inputs"]["files"], "collect_mir.py": "f" * 64}}
		manifest = {"schema_version": 2, "tool": "rustc-mir",
			"toolchain": graph.collect_mir.TOOLCHAIN, "timeout_seconds": 900,
			"requested_packages": [], "packages": [], "provenance": provenance}
		with self.assertRaisesRegex(ValueError, "verified source provenance"):
			graph.validate_semantic_manifest(manifest, Path("manifest.json"), "rustc-mir")

	def test_semantic_manifest_should_bind_package_metadata_to_workspace(self):
		with tempfile.TemporaryDirectory() as directory:
			root = Path(directory)
			package = {"package": "p", "owner": "pallet:wrong", "manifest": "pallets/p/Cargo.toml"}
			provenance = semantic_provenance("rustc-mir", graph.analysis_provenance.tree_fingerprint(root))
			command = graph.collect_mir.mir_command(package)
			manifest = {"schema_version": 2, "tool": "rustc-mir",
				"toolchain": graph.collect_mir.TOOLCHAIN, "timeout_seconds": 900,
				"requested_packages": ["p"], "provenance": provenance,
				"packages": [{**package, "status": "timeout", "artifact": "p.mir",
					"command": command, "input_fingerprint": graph.analysis_provenance.command_fingerprint(
						provenance, package, command)}]}
			workspace = [{"package": "p", "owner": "pallet:p", "manifest": "pallets/p/Cargo.toml"}]
			with mock.patch.object(graph.collect_rapx, "workspace_packages", return_value=workspace):
				with self.assertRaisesRegex(ValueError, "does not match the workspace"):
					graph.validate_semantic_manifest(manifest, root / "manifest.json", "rustc-mir", root)

	def test_rustc_mir_should_attach_cfg_ordering_to_matching_function(self):
		g = graph.Graph()
		g.node("function:trade", "function", name="trade", owner="pallet:hsm",
			file="pallets/hsm/src/pallet.rs", line=2)
		output = """fn <impl at pallets/hsm/src/pallet.rs:1:1: 1:20>::trade(_1: &T, _2: impl FnOnce(A) -> B) -> Result<(), E> {
    bb0: {
        _3 = StorageMap::<Prefix, Blake2, K, V>::insert(copy _1, copy _2) -> [return: bb1, unwind: bb3];
    }
    bb1: {
        _4 = <Currency as Mutate<Account>>::transfer(copy _1, copy _2) -> [return: bb2, unwind: bb3];
    }
    bb2: {
        _5 = StorageValue::<Other>::put(copy _4) -> [return: bb3, unwind: bb3];
    }
    bb3: { return; }
}
"""
		with tempfile.TemporaryDirectory() as directory:
			path = Path(directory) / "hsm.mir"
			path.write_text(output)
			self.assertEqual(graph.merge_rustc_mir(g, path, "pallet:hsm"), 3)
		self.assertTrue(g.nodes["function:trade"]["mir_write_before_external"])
		self.assertTrue(g.nodes["function:trade"]["mir_write_after_external"])
		self.assertEqual(g.nodes["semantic-analysis:rustc-mir:pallet:hsm"]["matched_functions"], 1)
		mir_edges = [edge for edge in g.edges if edge["kind"].startswith("mir-")]
		self.assertTrue(any(edge["kind"] == "mir-control-flow" for edge in mir_edges))
		self.assertTrue(any(edge["kind"] == "mir-unwind-flow" for edge in mir_edges))
		self.assertTrue(any(edge["target"] == "boundary:external-execution" for edge in mir_edges))

	def test_rustc_mir_should_reuse_exact_source_config_identity(self):
		g = graph.Graph()
		function = "function:request_fund"
		targets = {f"associated:pallet:liquidity-mining:Instance{index}:MultiCurrency" for index in (1, 2)}
		g.node(function, "function", name="request_fund", owner="pallet:liquidity-mining")
		for target in targets:
			instance = target.split(":")[-2]
			g.node(target, "associated-type", associated_type="MultiCurrency",
				config_trait="pallet_liquidity_mining::Config", config_instance=instance,
				runtime_instance=f"runtime-instance:{instance}", unresolved=False)
			g.edge(function, target, "dynamic-call", method="transfer",
				config_trait="pallet_liquidity_mining::Config", config_instance=instance)
		output = """fn request_fund() -> Result<(), E> {
    bb0: {
        _1 = <<T as pallet::Config<I>>::MultiCurrency as Currency<A>>::transfer(_2, _3) -> [return: bb1, unwind: bb2];
    }
    bb1: { return; }
    bb2: { resume; }
}
"""
		with tempfile.TemporaryDirectory() as directory:
			path = Path(directory) / "dispenser.mir"
			path.write_text(output)
			graph.merge_rustc_mir(g, path, "pallet:liquidity-mining")
		mir_edges = [edge for edge in g.edges if edge["kind"] == "mir-dynamic-call"]
		self.assertEqual({edge["target"] for edge in mir_edges}, targets)
		self.assertTrue(all(edge["config_trait"] == "pallet_liquidity_mining::Config"
			for edge in mir_edges))

	def test_rustc_mir_generic_fallback_should_exclude_unconfigured_declaration_node(self):
		g = graph.Graph()
		function = "function:adjust"
		g.node(function, "function", name="adjust", owner="pallet:liquidity-mining")
		g.node("associated:pallet:liquidity-mining:PriceAdjustment", "associated-type",
			associated_type="PriceAdjustment", config_trait="pallet_liquidity_mining::Config",
			config_instance=None, unresolved=False)
		targets = set()
		for index in (1, 2):
			instance = f"Instance{index}"
			target = f"associated:pallet:liquidity-mining:{instance}:PriceAdjustment"
			targets.add(target)
			g.node(target, "associated-type", associated_type="PriceAdjustment",
				config_trait="pallet_liquidity_mining::Config", config_instance=instance,
				runtime_instance=f"runtime-instance:{instance}", unresolved=False)
		output = """fn adjust() -> Result<(), E> {
    bb0: {
        _1 = <<T as pallet::Config<I>>::PriceAdjustment as Adjustment<A>>::get(_2) -> [return: bb1, unwind: bb2];
    }
    bb1: { return; }
    bb2: { resume; }
}
"""
		with tempfile.TemporaryDirectory() as directory:
			path = Path(directory) / "liquidity-mining.mir"
			path.write_text(output)
			graph.merge_rustc_mir(g, path, "pallet:liquidity-mining")
		self.assertEqual({edge["target"] for edge in g.edges if edge["kind"] == "mir-dynamic-call"}, targets)

	def test_runtime_mir_should_require_exact_source_file_for_same_named_functions(self):
		g = graph.Graph()
		root_call = "function:runtime/hydradx/src/lib.rs:call"
		runner_call = "function:runtime/hydradx/src/evm/runner.rs:call"
		g.node(root_call, "function", name="call", owner="runtime:hydradx",
			file="runtime/hydradx/src/lib.rs", line=100)
		g.node(runner_call, "function", name="call", owner="component:evm:runner",
			file="runtime/hydradx/src/evm/runner.rs", line=20)
		output = """fn <impl at runtime/hydradx/src/evm/runner.rs:10:1: 10:20>::call() -> Result<(), E> {
    bb0: {
        _1 = <Evm as hydradx_traits::evm::EVM<Result>>::call(_2) -> [return: bb1, unwind: bb2];
    }
    bb1: { return; }
    bb2: { resume; }
}
"""
		with tempfile.TemporaryDirectory() as directory:
			path = Path(directory) / "runtime.mir"
			path.write_text(output)
			graph.merge_rustc_mir(g, path, "runtime:hydradx")
		self.assertTrue(any(edge["source"] == runner_call and edge["kind"] == "has-mir-instance"
			for edge in g.edges))
		self.assertFalse(any(edge["source"] == root_call and edge["kind"] == "has-mir-instance"
			for edge in g.edges))
		instance = next(node for node in g.nodes.values() if node["kind"] == "mir-instance")
		self.assertEqual(instance["owner"], "component:evm:runner")

	def test_mir_ordering_should_follow_branches_without_conflating_paths(self):
		blocks = {
			0: {"operations": [], "successors": [1, 2]},
			1: {"operations": [{"kind": "storage-write"}], "successors": [3]},
			2: {"operations": [{"kind": "external-call"}], "successors": [3]},
			3: {"operations": [{"kind": "storage-write"}], "successors": []},
		}
		self.assertEqual(graph.mir_order_flags(blocks), (False, True))

	def test_mir_operations_should_cover_storage_nmap_and_evm(self):
		self.assertEqual(graph.mir_operation("StorageNMap::<Prefix, Keys, V>::insert(_1, _2)"), "storage-write")
		self.assertEqual(graph.mir_operation(
			"<T::Evm as hydradx_traits::evm::EVM<Result>>::call(_1, _2)"), "evm-call")
		self.assertEqual(graph.mir_operation("frame_support::storage::with_transaction::<(), E, F>(_1)"),
			"transaction-start")

	def test_contract_manifest_should_merge_aliases_abi_and_runtime_links(self):
		g = graph.Graph()
		graph.ensure_evm_selector(g, "supply(address,uint256)")
		payload = {"substrate_snapshot": {"block_hash": "0xabc"}, "contracts": [{"project": "aave-v3-deploy", "network": "hydration",
			"name": "Pool-Proxy-Hydration", "address": "0x1111111111111111111111111111111111111111",
			"artifact": "deployments/hydration/pool.json", "abi_signatures": ["supply(address,uint256)"]}],
			"runtime_configurations": [{"component": "pallet:liquidation", "storage": "liquidation.borrowingContract",
				"address": "0x1111111111111111111111111111111111111111"}],
			"observations": [{"project": "aave-v3-deploy", "network": "hydration",
				"address": "0x1111111111111111111111111111111111111111", "has_code": True,
				"implementation": "0x2222222222222222222222222222222222222222", "embedded_addresses": []}]}
		with tempfile.TemporaryDirectory() as directory:
			path = Path(directory) / "contracts.json"
			path.write_text(__import__("json").dumps(payload))
			self.assertEqual(graph.merge_contracts(g, path), 1)
		contract = "deployed-contract:aave-v3-deploy:hydration:0x1111111111111111111111111111111111111111"
		self.assertEqual(g.nodes[contract]["network"], "hydration")
		self.assertTrue(any(edge["source"] == "component:evm:aave_trade_executor" and edge["target"] == contract
			for edge in g.edges))
		self.assertTrue(any(node.get("signature") == "supply(address,uint256)" for node in g.nodes.values()))
		self.assertTrue(any(edge["kind"] == "selector-matches-contract-function" for edge in g.edges))
		self.assertTrue(g.nodes[contract]["has_code"])
		self.assertTrue(any(edge["source"] == "pallet:liquidation" and edge["target"] == contract
			and edge["kind"] == "runtime-configures-contract" for edge in g.edges))
		self.assertTrue(any(edge["source"] == contract and edge["kind"] == "proxy-implementation" for edge in g.edges))

	def test_contract_snapshot_diff_should_report_proxy_and_code_changes(self):
		before = {"observations": [{"project": "p", "network": "n", "address": "0x1",
			"bytecode_sha256": "old", "implementation": "0xa"}]}
		after = {"observations": [{"project": "p", "network": "n", "address": "0x1",
			"bytecode_sha256": "new", "implementation": "0xb"}]}
		result = snapshot_diff.diff(before, after)
		self.assertEqual(len(result["changed"]), 1)
		self.assertEqual(set(result["changed"][0]["changes"]), {"bytecode_sha256", "implementation"})

	def test_graph_diff_should_report_added_nodes_and_edges(self):
		before = {"nodes": [{"id": "a"}], "edges": []}
		after = {"nodes": [{"id": "a"}, {"id": "b"}],
			"edges": [{"source": "a", "target": "b", "kind": "calls"}]}
		result = graph_diff.diff(before, after)
		self.assertEqual(result["nodes_added"], ["b"])
		self.assertEqual({key: result["edges_added"][0][key] for key in ("source", "target", "kind")},
			{"source": "a", "target": "b", "kind": "calls"})
		self.assertEqual(result["nodes_changed"], [])
		self.assertEqual(result["edges_changed"], [])

	def test_graph_diff_should_prioritize_added_and_removed_security_edges(self):
		before = {"nodes": [], "edges": [
			{"source": "precompile:assets", "target": "boundary:external-execution",
				"kind": "external-execution"},
		]}
		after = {"nodes": [], "edges": [
			{"source": "origin:root", "target": "function:x", "kind": "authorizes-entry"},
			{"source": "a", "target": "b", "kind": "contains"},
		]}
		result = graph_diff.diff(before, after)
		self.assertEqual(len(result["review_edges_added"]), 1)
		self.assertEqual(len(result["review_edges_removed"]), 1)
		markdown = graph_diff.markdown(result)
		self.assertIn("New review-priority edges", markdown)
		self.assertIn("Removed review-priority edges", markdown)

	def test_graph_diff_should_collapse_line_variants_without_type_error(self):
		before = {"nodes": [], "edges": []}
		after = {"nodes": [], "edges": [
			{"source": "a", "target": "b", "kind": "binding-resolves-to", "line": 10},
			{"source": "a", "target": "b", "kind": "binding-resolves-to"},
			{"source": "a", "target": "b", "kind": "binding-resolves-to", "method": "resolve"},
		]}
		result = graph_diff.diff(before, after)
		self.assertEqual(len(result["edges_added"]), 2)
		self.assertEqual(next(edge["location"]["line"] for edge in result["edges_added"]
			if edge.get("method") is None), 10)

	def test_graph_diff_should_ignore_location_only_changes(self):
		before = {"nodes": [{"id": "a", "kind": "function", "file": "old.rs", "line": 1}], "edges": [
			{"source": "a", "target": "b", "kind": "calls", "method": "run",
				"file": "old.rs", "line": 10},
		]}
		after = {"nodes": [{"id": "a", "kind": "function", "file": "new.rs", "line": 2}], "edges": [
			{"source": "a", "target": "b", "kind": "calls", "method": "run",
				"file": "new.rs", "line": 20},
		]}
		result = graph_diff.diff(before, after)
		self.assertEqual(result["edges_added"], [])
		self.assertEqual(result["edges_removed"], [])
		self.assertEqual(result["edges_changed"], [])
		self.assertEqual(result["nodes_changed"], [])

	def test_graph_diff_should_report_node_metadata_changes(self):
		before = {"nodes": [{"id": "a", "kind": "function", "transactional": False, "line": 1}],
			"edges": []}
		after = {"nodes": [{"id": "a", "kind": "function", "transactional": True, "line": 2}],
			"edges": []}
		result = graph_diff.diff(before, after)
		self.assertEqual(len(result["nodes_changed"]), 1)
		self.assertFalse(result["nodes_changed"][0]["before"]["transactional"])
		self.assertTrue(result["nodes_changed"][0]["after"]["transactional"])
		self.assertIn("Changed nodes", graph_diff.markdown(result))

	def test_graph_diff_should_preserve_semantic_edge_variants(self):
		before = {"nodes": [], "edges": [
			{"source": "function:a", "target": "storage:x", "kind": "storage-access",
				"operation": "get", "line": 10},
			{"source": "function:a", "target": "storage:x", "kind": "storage-access",
				"operation": "put", "value": "old", "enforcement": "observation", "line": 20},
		]}
		after = {"nodes": [], "edges": [
			{"source": "function:a", "target": "storage:x", "kind": "storage-access",
				"operation": "get", "line": 11},
			{"source": "function:a", "target": "storage:x", "kind": "storage-access",
				"operation": "mutate", "value": "new", "enforcement": "runtime", "line": 21},
		]}
		result = graph_diff.diff(before, after)
		self.assertEqual(result["edges_added"], [])
		self.assertEqual(result["edges_removed"], [])
		self.assertEqual(len(result["edges_changed"]), 1)
		self.assertEqual([edge["operation"] for edge in result["edges_changed"][0]["before"]], ["put"])
		self.assertEqual([edge["operation"] for edge in result["edges_changed"][0]["after"]], ["mutate"])
		self.assertEqual(result["edges_changed"][0]["before"][0]["value"], "old")
		self.assertEqual(result["edges_changed"][0]["after"][0]["value"], "new")
		self.assertEqual(result["edges_changed"][0]["before"][0]["enforcement"], "observation")
		self.assertEqual(result["edges_changed"][0]["after"][0]["enforcement"], "runtime")

	def test_graph_diff_should_describe_changed_priority_edge_semantics(self):
		before = {"nodes": [], "edges": [{
			"source": "operation:withdraw", "target": "invariant:shares", "kind": "enforces",
			"enforcement": "observation",
		}]}
		after = {"nodes": [], "edges": [{
			"source": "operation:withdraw", "target": "invariant:shares", "kind": "enforces",
			"enforcement": "runtime",
		}]}
		result = graph_diff.diff(before, after)
		self.assertEqual(len(result["review_edges_changed"]), 1)
		markdown = graph_diff.markdown(result)
		self.assertIn("Changed review-priority edges", markdown)
		self.assertIn("before: enforcement=observation", markdown)
		self.assertIn("after: enforcement=runtime", markdown)

	def test_coverage_comparison_should_enforce_regression_budgets(self):
		thresholds = {"regression": {
			"maximum_drop": {"entrypoints": 2},
			"maximum_increase": {"unresolved_targets": 1},
		}}
		result = graph_diff.compare_coverage(
			{"entrypoints": 100, "unresolved_targets": 10},
			{"entrypoints": 97, "unresolved_targets": 12},
			thresholds,
		)
		self.assertEqual(len(result["regressions"]), 2)
		self.assertEqual(result["changes"], {"entrypoints": -3, "unresolved_targets": 2})

	def test_coverage_comparison_should_enforce_absolute_thresholds(self):
		result = graph_diff.compare_coverage(
			{"nodes": 100, "unresolved_targets": 5},
			{"nodes": 99, "unresolved_targets": 6},
			{"minimum": {"nodes": 100}, "maximum": {"unresolved_targets": 5}},
		)
		self.assertEqual(len(result["regressions"]), 2)

	def test_coverage_comparison_should_enforce_exact_inventory_classification(self):
		expected = {"migration-not-configured": 2}
		result = graph_diff.compare_coverage(
			{"inventory_only_targets_by_reason": expected},
			{"inventory_only_targets_by_reason": {"migration-not-configured": 3}},
			{"exact": {"inventory_only_targets_by_reason": expected}},
		)
		self.assertEqual(len(result["regressions"]), 1)

	def test_dangerous_rules_should_detect_evm_frame_and_state_ordering(self):
		g = graph.Graph()
		g.node("function:f", "function", operations=[
			{"kind": "storage-write"}, {"kind": "external-call"}, {"kind": "storage-write"},
		])
		items = graph.dangerous_interactions(g, [], [[
			"boundary:evm-execution", "precompile:dispatch", "boundary:frame-dispatch",
		]])
		self.assertEqual({item["rule"] for item in items},
			{"evm-precompile-frame-dispatch", "state-write-around-external-call"})


class HydrationRuntimeGraphTests(unittest.TestCase):
	@classmethod
	def setUpClass(cls):
		cls.graph = graph.scan(MODULE.parents[2])
		cls.components = graph.resolved_component_edges(cls.graph, graph.component_edges(cls.graph))

	def test_route_executor_should_reach_evm_through_hsm(self):
		paths = graph.bounded_paths(self.components, {"boundary:evm-execution"})
		self.assertIn(
			["pallet:route-executor", "pallet:hsm", "boundary:evm-execution"],
			paths,
		)

	def test_evm_should_reach_frame_dispatch_precompile(self):
		paths = graph.bounded_paths(self.components, {"boundary:frame-dispatch"})
		self.assertIn(
			["boundary:evm-execution", "precompile:dispatch", "boundary:frame-dispatch"],
			paths,
		)

	def test_runtime_evm_selectors_should_be_first_class_nodes(self):
		signatures = {node["signature"] for node in self.graph.nodes.values() if node["kind"] == "evm-signature"}
		self.assertTrue({"supply(address,uint256,address,uint16)", "withdraw(address,uint256,address)",
			"balanceOf(address)"}.issubset(signatures))
		precompile_entrypoints = [node for node in self.graph.nodes.values()
			if node.get("entrypoint_kind") == "precompile-selector"]
		selector_targets = {edge["source"]: edge["target"] for edge in self.graph.edges
			if edge["kind"] == "dispatches-evm-selector"}
		self.assertTrue(precompile_entrypoints)
		self.assertTrue(all(entrypoint["id"] in selector_targets for entrypoint in precompile_entrypoints))
		self.assertTrue(all(selector_targets[entrypoint["id"]].startswith("evm-selector:0x")
			for entrypoint in precompile_entrypoints))

	def test_internal_executor_calls_should_reach_evm_boundary(self):
		edges = {(edge["source"], edge["target"], edge["kind"], edge.get("method"))
			for edge in self.components}
		self.assertIn(("component:runtime:gigahdx", "component:evm:executor", "direct-call", "call"), edges)
		self.assertIn(("component:evm:erc20_currency", "component:evm:executor", "direct-call", "call"), edges)
		self.assertIn(("component:evm:erc20_currency", "component:evm:executor", "direct-call", "view"), edges)
		self.assertIn(("component:evm:executor", "boundary:evm-execution", "enters-evm", None), edges)

	def test_generated_selector_aliases_should_link_actual_evm_callsites(self):
		def encoded(owner: str) -> set[str]:
			return {signature for edge in self.graph.edges
				if self.graph.nodes[edge["source"]].get("owner") == owner
				and edge["kind"] == "encodes-evm-selector"
				for signature in self.graph.nodes[edge["target"]].get("signatures", [])}

		self.assertTrue({"supply(address,uint256,address,uint16)", "withdraw(address,uint256,address)"}
			.issubset(encoded("component:runtime:gigahdx")))
		self.assertTrue({"flashLoan(address,address,uint256,bytes)", "maxFlashLoan(address)",
			"getFacilitatorBucket(address)"}.issubset(encoded("pallet:hsm")))

	def test_chainlink_precompile_should_have_one_canonical_dynamic_identity(self):
		chainlink = [node for node in self.graph.nodes.values()
			if node["id"].startswith("precompile:runtime:chainlink")]
		self.assertEqual([node["id"] for node in chainlink], ["precompile:runtime:chainlink_adapter"])
		self.assertTrue(chainlink[0]["dynamic_address"])
		self.assertEqual(chainlink[0]["address_predicate"], "is_oracle_address")
		self.assertTrue(any(node.get("owner") == chainlink[0]["id"]
			for node in self.graph.nodes.values() if node["kind"] == "function"))
		execute = next(node for node in self.graph.nodes.values()
			if node.get("owner") == chainlink[0]["id"] and node.get("name") == "execute")
		dispatched = {self.graph.nodes[edge["target"]]["signatures"][0] for edge in self.graph.edges
			if edge["kind"] == "dispatches-evm-selector"
			and self.graph.nodes[edge["source"]].get("entrypoint_kind") == "precompile-selector"
			and any(link["kind"] == "enters-function" and link["source"] == edge["source"]
				and link["target"] == execute["id"] for link in self.graph.edges)}
		self.assertEqual(dispatched, {"getAnswer(uint256)", "latestAnswer()", "decimals()"})
		self.assertFalse(any(edge["source"] == execute["id"] and edge["kind"] == "encodes-evm-selector"
			for edge in self.graph.edges))

	def test_multicurrency_precompile_should_link_imported_erc20_selectors(self):
		execute = next(node for node in self.graph.nodes.values()
			if node.get("owner") == "precompile:runtime:multicurrency" and node.get("name") == "execute")
		entrypoints = {edge["source"] for edge in self.graph.edges
			if edge["kind"] == "enters-function" and edge["target"] == execute["id"]
			and self.graph.nodes[edge["source"]].get("entrypoint_kind") == "precompile-selector"}
		self.assertEqual(len(entrypoints), 11)
		selectors = {self.graph.nodes[entrypoint]["signature"] for entrypoint in entrypoints}
		self.assertTrue({"balanceOf(address)", "transfer(address,uint256)", "transferFrom(address,address,uint256)",
			"mint(address,uint256)", "burn(uint256)"}.issubset(selectors))
		self.assertEqual({edge["source"] for edge in self.graph.edges
			if edge["kind"] == "dispatches-evm-selector" and edge["source"] in entrypoints}, entrypoints)
		self.assertFalse(any(edge["source"] == execute["id"] and edge["kind"] == "encodes-evm-selector"
			for edge in self.graph.edges))

	def test_precompile_utilities_and_build_helpers_should_not_be_runtime_entrypoints(self):
		self.assertFalse(any(node.get("owner") == "precompile:utils" and node.get("entrypoint_kind")
			for node in self.graph.nodes.values()))
		self.assertFalse(any(node["id"].startswith("component:evm:evm-utility")
			for node in self.graph.nodes.values()))
		self.assertFalse(any(node.get("file") == "runtime/hydradx/src/helpers.rs"
			for node in self.graph.nodes.values()))
		self.assertFalse(any(node.get("file") == "runtime/hydradx/src/lib.rs"
			and node.get("name") in {"benchmark_metadata", "dispatch_benchmark"}
			for node in self.graph.nodes.values()))

	def test_external_test_and_test_utility_modules_should_not_be_scanned(self):
		excluded = {
			"external/pallet_message_queue/integration_test.rs",
			"external/pallet_xcm/xcm_helpers.rs",
		}
		self.assertFalse(any(node.get("file") in excluded for node in self.graph.nodes.values()))

	def test_benchmark_runtime_bindings_should_not_replace_production_bindings(self):
		bindings = [(edge["source"], self.graph.nodes[edge["target"]].get("value", ""))
			for edge in self.graph.edges if edge["kind"] == "config-binding"]
		self.assertFalse(any(source == "pallet:message-queue" and "NoopMessageProcessor" in value
			for source, value in bindings))
		self.assertTrue(any(source == "pallet:message-queue" and "ProcessXcmWithBreaker" in value
			for source, value in bindings))

	def test_active_runtime_associated_targets_should_all_resolve(self):
		self.assertFalse(any(node.get("unresolved") for node in self.graph.nodes.values()))
		associated_edge_kinds = {"dynamic-call", "configured-origin", "runtime-config-read",
			"runtime-config-type-reference", "weight-evaluation"}
		active_edges = [edge for edge in self.graph.edges if edge["kind"] in associated_edge_kinds
			and self.graph.nodes.get(edge["target"], {}).get("associated_type")
			and graph.edge_runtime_active(self.graph, edge)]
		self.assertGreater(len(active_edges), 100)
		self.assertFalse([(edge["source"], edge["target"], edge["kind"]) for edge in active_edges
			if not edge.get("config_trait") or not self.graph.nodes[edge["target"]].get("config_trait")])
		self.assertFalse(any(node["id"].startswith(("associated:component:evm:runner:",
			"associated:component:evm:executor:", "associated:component:evm:aave_trade_executor:"))
			for node in self.graph.nodes.values()))
		evm_files = {
			"runtime/hydradx/src/evm/runner.rs",
			"runtime/hydradx/src/evm/executor.rs",
			"runtime/hydradx/src/evm/aave_trade_executor.rs",
		}
		evm_edges = [edge for edge in self.graph.edges if edge["kind"] in associated_edge_kinds
			and edge.get("file") in evm_files and self.graph.nodes[edge["target"]].get("associated_type")
			in {"AccountProvider", "AddressMapping", "BlockGasLimit", "ChainId", "FeeCalculator",
				"GasWeightMapping", "PrecompilesValue"}]
		self.assertGreater(len(evm_edges), 10)
		self.assertTrue(all(edge.get("config_trait") == "pallet_evm::Config"
			and self.graph.nodes[edge["target"]].get("config_trait") == "pallet_evm::Config"
			for edge in evm_edges))

	def test_runtime_and_generic_evm_ufcs_calls_should_resolve_exactly(self):
		files = {
			"runtime/hydradx/src/lib.rs",
			"runtime/hydradx/src/gigahdx.rs",
			"runtime/hydradx/src/evm/permit.rs",
		}
		for source in files:
			edges = [edge for edge in self.graph.edges if edge.get("file") == source
				and edge.get("config_trait") == "pallet_evm::Config"
				and self.graph.nodes.get(edge["target"], {}).get("associated_type")]
			self.assertGreater(len(edges), 0, source)
			self.assertTrue(all(self.graph.nodes[edge["target"]].get("config_trait") == "pallet_evm::Config"
				for edge in edges), source)

	def test_explicit_child_config_should_not_resolve_to_redeclared_parent_type(self):
		edges = [edge for edge in self.graph.edges if edge.get("file") == "pallets/dispenser/src/lib.rs"
			and edge.get("method") in {"balance", "transfer"}
			and edge["target"].startswith("associated:")
			and self.graph.nodes.get(edge["target"], {}).get("associated_type") == "Currency"]
		self.assertGreater(len(edges), 0)
		self.assertEqual({edge["target"] for edge in edges}, {"associated:pallet:dispenser:Currency"})

	def test_collective_config_should_resolve_to_exact_runtime_instance(self):
		self.assertNotIn("pallet:collective-technical-committee", self.graph.nodes)
		for associated_type in ("Consideration", "DefaultVote", "DisapproveOrigin", "KillOrigin",
			"SetMembersOrigin"):
			node = self.graph.nodes[f"associated:pallet:collective:Instance2:{associated_type}"]
			self.assertFalse(node["unresolved"])
			self.assertEqual(node["config_trait"], "pallet_collective::Config")
			self.assertEqual(node["config_instance"], "Instance2")
			self.assertEqual(node["runtime_instance"], "runtime-instance:TechnicalCommittee")

	def test_gat_runtime_binding_should_resolve_try_call_currency(self):
		target = "associated:pallet:transaction-multi-payment:TryCallCurrency"
		node = self.graph.nodes[target]
		self.assertFalse(node["unresolved"])
		self.assertIn("TryConvert<&'a", node["trait_bounds"])
		self.assertTrue(any(edge["source"] == target and edge["target"] == "component:runtime:assets"
			and edge["kind"] == "binding-resolves-to" for edge in self.graph.edges))

	def test_inventory_only_associated_targets_should_be_explicit_and_excluded(self):
		expected = {
			"associated:pallet:aura:PalletPrefix": "migration-not-configured",
			"associated:pallet:cumulus-pallet-xcmp-queue:ChannelList": "migration-not-configured",
			"associated:pallet:nft:Permissions": "component-not-instantiated",
			"associated:pallet:session:historical:FullIdentificationOf": "config-trait-not-implemented",
			"associated:pallet:xcm-rate-limiter:CurrencyIdConvert": "component-not-instantiated",
			"associated:pallet:xcm-rate-limiter:RelayBlockNumberProvider": "component-not-instantiated",
		}
		inventory_only = {node["id"]: node.get("runtime_activity_reason") for node in self.graph.nodes.values()
			if node.get("runtime_active") is False and node.get("kind") == "associated-type"}
		self.assertEqual(inventory_only, expected)
		for projection in ("execution", "callback"):
			self.assertFalse(any(edge["source"] in expected or edge["target"] in expected
				for edge in graph.projected_edges(self.graph, projection)))

	def test_configured_xcm_migration_helper_should_remain_runtime_active(self):
		migration_file = "external/pallet_xcm/migration.rs"
		helper = next(node for node in self.graph.nodes.values()
			if node.get("kind") == "function" and node.get("file") == migration_file
			and node.get("name") == "migrate_data_to_xcm_version")
		self.assertTrue(helper["runtime_active"])
		calls = [edge for edge in self.graph.edges if edge["target"] == helper["id"]
			and edge.get("resolution") == "source-local-migration"]
		self.assertEqual(len(calls), 1)
		self.assertTrue(self.graph.nodes[calls[0]["source"]]["runtime_active"])
		self.assertTrue(graph.edge_runtime_active(self.graph, calls[0]))

	def test_active_nested_module_functions_should_not_inherit_phantom_configs(self):
		active = [node for node in self.graph.nodes.values() if node.get("kind") == "function" and (
			node.get("file") == "external/cumulus_pallet_aura_ext/consensus_hook.rs"
			or node.get("file") == "pallets/broadcast/src/types.rs")]
		self.assertGreaterEqual(len(active), 4)
		self.assertTrue(all(node.get("runtime_active") is not False for node in active))
		self.assertTrue(all(node.get("source_config_trait") in {
			"cumulus_pallet_aura_ext::Config", "pallet_broadcast::Config"} for node in active))

	def test_call_permit_should_record_nonce_write_before_evm_subcall(self):
		candidates = [node for node in self.graph.nodes.values()
			if node["kind"] == "audit-candidate" and "precompiles/call-permit/src/lib.rs" in node["id"]]
		self.assertEqual(len(candidates), 1)
		self.assertTrue(candidates[0]["storage_before"])
		self.assertFalse(candidates[0]["storage_after"])

	def test_asset_registry_should_define_runtime_asset_kinds(self):
		for kind in ("Token", "XYK", "StableSwap", "Bond", "External", "Erc20"):
			self.assertEqual(self.graph.nodes[f"asset-kind:{kind}"]["source"],
				"pallets/asset-registry/src/types.rs")

	def test_route_executor_should_resolve_all_runtime_amm_backends(self):
		targets = {edge["target"] for edge in self.components
			if edge["source"] == "pallet:route-executor" and edge["kind"] == "resolved-call"}
		self.assertTrue({"pallet:omnipool", "pallet:stableswap", "pallet:xyk", "pallet:lbp", "pallet:hsm",
			"component:evm:aave_trade_executor"}.issubset(targets))

	def test_evm_runner_binding_should_only_resolve_to_outer_callback(self):
		targets = {edge["target"] for edge in self.graph.edges
			if edge["source"] == "associated:pallet:evm:Runner"
			and edge["kind"] == "binding-resolves-to"}
		self.assertEqual(targets, {"component:evm:runner"})

	def test_warehouse_liquidity_mining_instances_should_share_canonical_owner(self):
		expected = {"runtime-instance:OmnipoolWarehouseLM", "runtime-instance:XYKWarehouseLM"}
		instances = {edge["source"] for edge in self.graph.edges
			if edge["kind"] == "instantiates" and edge["target"] == "pallet:liquidity-mining"}
		self.assertTrue(expected.issubset(instances))
		self.assertTrue(all(self.graph.nodes[instance]["owner"] == "pallet:liquidity-mining"
			for instance in expected))
		self.assertNotIn("pallet:warehouse-liquidity-mining", self.graph.nodes)

	def test_asset_components_should_cover_balances_orml_tokens_and_erc20(self):
		edges = {(edge["source"], edge["target"], edge["kind"]) for edge in self.components}
		self.assertIn(("pallet:balances", "asset-backend:balances", "uses-asset-backend"), edges)
		self.assertIn(("pallet:orml-tokens", "asset-backend:tokens", "uses-asset-backend"), edges)
		self.assertNotIn("pallet:tokens", self.graph.nodes)
		self.assertIn(("component:evm:erc20_currency", "asset-backend:erc20", "uses-asset-backend"), edges)
		self.assertIn(("pallet:stableswap", "asset-kind:StableSwap", "issues-asset-kind"), edges)

	def test_xcm_should_connect_message_and_asset_boundaries(self):
		edges = {(edge["source"], edge["target"], edge["kind"]) for edge in self.components}
		self.assertIn(("component:xcm:router", "boundary:xcm-outbound", "sends-xcm"), edges)
		self.assertIn(("boundary:xcm-inbound", "component:xcm:executor", "receives-xcm"), edges)
		self.assertIn(("component:xcm:asset-transactor", "asset-backend:xcm", "uses-asset-backend"), edges)

	def test_runtime_entrypoints_should_include_extrinsics_hooks_callbacks_and_evm(self):
		kinds = {node.get("entrypoint_kind") for node in self.graph.nodes.values() if node["kind"] == "entrypoint"}
		self.assertTrue({"extrinsic", "runtime-hook", "runtime-callback", "precompile-selector",
			"precompile-dispatch", "evm-adapter", "xcm-inbound", "runtime-api", "unsigned-validation",
			"offchain-worker", "runtime-migration", "try-runtime"}.issubset(kinds))
		signatures = {node.get("signature") for node in self.graph.nodes.values()
			if node.get("entrypoint_kind") == "precompile-selector"}
		self.assertIn("DOMAIN_SEPARATOR()", signatures)
		entrypoints = [node for node in self.graph.nodes.values() if node["kind"] == "entrypoint"]
		self.assertFalse(any("/weights.rs:" in node["id"] or "/weights/" in node["id"] for node in entrypoints))
		self.assertFalse(any(node.get("method") in {"setup_set_code_requirements", "verify_set_code"}
			for node in entrypoints))
		self.assertFalse(any(node.get("method") == "init_omnipool" for node in entrypoints))

	def test_associated_type_resolution_should_not_cross_pallet_scopes(self):
		g = graph.Graph()
		g.node("associated:pallet:a:Handler", "associated-type", associated_type="Handler", unresolved=True)
		g.node("associated:pallet:b:Handler", "associated-type", associated_type="Handler", unresolved=True)
		g.edge("associated:pallet:a:Handler", "pallet:target", "binding-resolves-to")
		graph.enrich_unique_type_resolutions(g)
		inferred = [edge for edge in g.edges if edge["source"] == "associated:pallet:b:Handler"]
		self.assertEqual(inferred, [])
		self.assertTrue(g.nodes["associated:pallet:b:Handler"]["unresolved"])

	def test_unresolved_targets_should_retain_ambiguity_and_candidates(self):
		g = graph.Graph()
		g.node("associated:pallet:a:Handler", "associated-type", associated_type="Handler", unresolved=True)
		g.node("pallet:first", "pallet")
		g.node("pallet:second", "pallet")
		g.edge("associated:pallet:a:Handler", "pallet:first", "binding-resolves-to")
		g.edge("associated:pallet:a:Handler", "pallet:second", "binding-resolves-to")
		graph.classify_unresolved(g)
		node = g.nodes["associated:pallet:a:Handler"]
		self.assertEqual(node["ambiguity_reason"], "multiple-runtime-targets")
		self.assertEqual(node["candidate_targets"], ["pallet:first", "pallet:second"])

	def test_inventory_only_associated_calls_should_remain_in_raw_graph_but_not_callback_projection(self):
		g = graph.Graph()
		g.node("pallet:inactive", "pallet", runtime_active=False)
		g.node("function:inactive", "function", owner="pallet:inactive", runtime_active=False)
		g.node("associated:pallet:inactive:Handler", "associated-type", owner="pallet:inactive",
			associated_type="Handler", unresolved=True, runtime_active=False,
			runtime_activity_reason="component-not-instantiated")
		g.edge("function:inactive", "associated:pallet:inactive:Handler", "dynamic-call", method="call")
		graph.classify_unresolved(g)
		self.assertEqual(len(g.edges), 1)
		self.assertEqual(graph.projected_edges(g, "callback"), [])
		self.assertEqual(len(graph.projected_edges(g, "callback", active_only=False)), 1)
		self.assertFalse(g.nodes["associated:pallet:inactive:Handler"]["unresolved"])
		self.assertEqual(g.nodes["associated:pallet:inactive:Handler"]["resolution"], "inventory-only")

	def test_incomplete_runtime_inventory_should_not_guess_that_config_is_inactive(self):
		g = graph.Graph()
		g.node("pallet:unknown", "pallet")
		g.node("function:unknown", "function", owner="pallet:unknown",
			source_config_trait="pallet_unknown::Config")
		g.node("associated:pallet:unknown:Handler", "associated-type", owner="pallet:unknown",
			associated_type="Handler", config_trait="pallet_unknown::Config", unresolved=True)
		g.edge("function:unknown", "associated:pallet:unknown:Handler", "dynamic-call", method="call")
		graph.classify_runtime_activity(g, set(), {}, False)
		graph.classify_unresolved(g)
		self.assertTrue(g.nodes["associated:pallet:unknown:Handler"]["unresolved"])
		self.assertNotIn("runtime_active", g.nodes["associated:pallet:unknown:Handler"])

	def test_associated_getters_should_be_config_values_not_callbacks(self):
		g = graph.Graph()
		g.node("associated:pallet:a:DbWeight", "associated-type", associated_type="DbWeight", unresolved=True,
			associated_role="config-value", trait_bounds="Get<RuntimeDbWeight>")
		g.edge("function:a", "associated:pallet:a:DbWeight", "dynamic-call", method="get")
		graph.normalize_config_value_calls(g)
		self.assertEqual(g.nodes["associated:pallet:a:DbWeight"]["kind"], "runtime-config-value")
		self.assertEqual(g.edges[0]["kind"], "runtime-config-read")

	def test_config_value_normalization_should_rewrite_edges_added_after_first_pass(self):
		g = graph.Graph()
		target = "associated:pallet:a:DbWeight"
		g.node(target, "associated-type", associated_type="DbWeight", unresolved=True,
			associated_role="config-value", trait_bounds="Get<RuntimeDbWeight>")
		g.edge("function:source", target, "dynamic-call", method="get")
		graph.normalize_config_value_calls(g)
		g.edge("mir-operation:read", target, "mir-dynamic-call", method="get")
		graph.normalize_config_value_calls(g)
		self.assertEqual(g.nodes[target]["kind"], "runtime-config-value")
		self.assertEqual([edge["kind"] for edge in g.edges],
			["runtime-config-read", "runtime-config-read"])

	def test_layered_visualizations_should_be_deterministic(self):
		with tempfile.TemporaryDirectory() as directory:
			out = Path(directory)
			graph.write_outputs(self.graph, out)
			paths = [out / "graph-scale.svg"] + [out / "focused" / name for name in
				("component-dependencies.svg", "execution-boundaries.svg", "token-flows.svg")]
			first = [path.read_bytes() for path in paths]
			graph.write_outputs(self.graph, out)
			self.assertEqual(first, [path.read_bytes() for path in paths])

	def test_historical_component_paths_should_remain_reachable(self):
		fixtures = __import__("json").loads(MODULE.with_name("historical-interactions.json").read_text())
		paths = graph.bounded_paths(self.components,
			{"boundary:evm-execution", "boundary:frame-dispatch"})
		for required in fixtures["required_component_paths"]:
			self.assertIn(required, paths)
		properties = graph.historical_property_status(self.graph, self.components)
		for required in fixtures["required_runtime_properties"]:
			self.assertIn(required, properties)
			self.assertTrue(properties[required], required)

	def test_state_and_asset_semantics_should_be_first_class(self):
		kinds = {node["kind"] for node in self.graph.nodes.values()}
		self.assertIn("state-invariant", kinds)
		self.assertIn("asset-operation", kinds)
		self.assertTrue(any(edge["kind"] == "affects-invariant" for edge in self.graph.edges))
		self.assertTrue(any(edge["kind"] == "asset-operation" for edge in self.graph.edges))


if __name__ == "__main__":
	unittest.main()
