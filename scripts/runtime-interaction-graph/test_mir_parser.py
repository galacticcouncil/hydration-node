import importlib.util
import unittest
from pathlib import Path


MODULE = Path(__file__).with_name("mir_parser.py")
SPEC = importlib.util.spec_from_file_location("mir_parser", MODULE)
mir = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(mir)


class MirParserTests(unittest.TestCase):
	def test_parser_should_separate_instances_and_normal_from_unwind_flow(self):
		text = """fn pallet::<impl at pallets/example/src/lib.rs:10:1: 20:2>::run(_1: T) -> () {
    bb0: {
        _2 = pallet_other::Pallet::<T>::execute(copy _1) -> [return: bb1, unwind: bb2];
    }
    bb1: {
        _3 = StorageMap::<Prefix, Blake2, K, V>::insert(copy _1, copy _2) -> [return: bb3, unwind: bb2];
    }
    bb2: {
        _4 = StorageValue::<Recovery>::put(copy _1) -> [return: bb3, unwind continue];
    }
    bb3: { return; }
}
"""
		instances = mir.parse(text)
		self.assertEqual(len(instances), 1)
		instance = instances[0]
		self.assertEqual(instance["name"], "run")
		self.assertEqual(instance["source_file"], "pallets/example/src/lib.rs")
		self.assertEqual(instance["blocks"][0]["normal_successors"], [1])
		self.assertEqual(instance["blocks"][0]["unwind_successors"], [2])
		self.assertEqual(instance["blocks"][0]["operations"][0]["kind"], "call")
		self.assertEqual(mir.order_flags(instance["blocks"]), (False, False))
		self.assertEqual(mir.order_flags(instance["blocks"], "unwind_successors"), (False, False))

	def test_instance_ids_should_distinguish_monomorphized_symbols(self):
		first = "pallet::<impl at pallets/example/src/lib.rs:1:1: 2:2>::run::<u32>"
		second = "pallet::<impl at pallets/example/src/lib.rs:1:1: 2:2>::run::<u64>"
		self.assertNotEqual(mir.instance_id(first), mir.instance_id(second))


if __name__ == "__main__":
	unittest.main()
