import argparse
import json
import os
import subprocess

from collections import defaultdict
from dataclasses import dataclass, field

HYDRA_REF_VALUES_LOCATION = ".maintain/bench-check/hydradx-bench-data.json"

DIFF_MARGIN = 10  # percent

COMMAND = [
    "cargo",
    "run",
    "--release",
    "--features=runtime-benchmarks",
    "--manifest-path=node/Cargo.toml",
    "--",
    "benchmark",
    "--chain=dev",
    "--steps=5",
    "--repeat=20",
    "--extrinsic=*",
    "--execution=wasm",
    "--wasm-execution=compiled",
    "--heap-pages=4096",
]

PALLETS = ["amm", "exchange", "transaction_multi_payment"]

parser = argparse.ArgumentParser()
parser.add_argument(
    "--include-db-benchmark",
    dest="db_benchmark",
    action="store_true",
    help="Perform Substrate Database benchmark",
)
parser.add_argument(
    "--exclude-pallet-benchmark",
    dest="only_db_benchmark",
    action="store_true",
    help="Skip pallets benchmarks",
)
parser.add_argument(
    "--substrate-repo-path",
    dest="sb_repo",
    type=str,
    required=False,
    help="Substrate repository path (cloned if not provided)",
)
args = parser.parse_args()


class Benchmark:
    def __init__(self, pallet: str, command: [str], ref_value: float, extrinsics: list):

        # refactor the usage of this protected members so not called directly
        self._pallet = pallet
        self._stdout = None
        self._command = command
        self._ref_value = ref_value
        self._extrinsics = extrinsics

        self._extrinsics_results = []

        self._total_time = 0

        self._completed = False
        self._acceptable = False
        self._rerun = False

    def run(self, rerun=False):
        result = subprocess.run(self._command, capture_output=True)
        # TODO: check the return code

        self._stdout = result.stdout

        lines = list(map(lambda x: x.decode(), self._stdout.split(b"\n")))

        for idx, line in enumerate(lines):
            if line.startswith("Pallet:"):
                info = line.split(",")
                # pallet_name = info[0].split(":")[1].strip()[1:-1]
                extrinsic = info[1].split(":")[1].strip()[1:-1]
                if extrinsic in self._extrinsics:
                    self._extrinsics_results.append(
                        process_extrinsic(lines[idx + 1 : idx + 21])
                    )

        self._total_time = sum(list(map(lambda x: float(x), self._extrinsics_results)))
        hydra_margin = int(self._ref_value * DIFF_MARGIN / 100)

        diff = int(self._ref_value - self._total_time)

        self._acceptable = diff >= -hydra_margin
        self._rerun = rerun


@dataclass
class Config:
    do_db_bench: bool = False
    substrate_repo_path: str = "./substrate"
    do_pallet_bench: bool = True
    pallets: [str] = field(default_factory=lambda: PALLETS)


def load_hydra_values(filename):
    with open(filename, "r") as f:
        return json.load(f)


def process_extrinsic(data):
    for entry in data:
        if entry.startswith("Time"):
            return float(entry.split(" ")[-1])


def prepare_benchmarks(config: Config, reference_values: dict):

    benchmarks = []

    for pallet in config.pallets:
        command = COMMAND + [f"--pallet={pallet}"]
        hydra_data = reference_values[pallet]
        hydra_value = sum(list(map(lambda x: float(x), hydra_data.values())))
        benchmarks.append(Benchmark(pallet, command, hydra_value, hydra_data.keys()))

    return benchmarks


def run_benchmarks(benchmarks: [Benchmark], rerun=False):
    # Note : this can be simplified into one statement
    if rerun:
        [bench.run(rerun) for bench in benchmarks if bench._acceptable is False]
    else:
        print("Running benchmarks - this may take a while...")
        [bench.run() for bench in benchmarks]


def show_pallet_result(pallet_result: Benchmark):
    pallet = pallet_result._pallet
    hydra = pallet_result._ref_value
    current = pallet_result._total_time

    hydra_margin = int(hydra * DIFF_MARGIN / 100)

    diff = int(hydra - current)

    note = "OK" if diff >= -hydra_margin else "FAILED"

    diff = f"{diff}"
    times = f"{hydra:.2f} vs {current:.2f}"

    rerun = "*" if pallet_result._rerun else ""

    print(f"{pallet:<25}| {times:^25} | {diff:^13} | {note:^10} | {rerun:^10}")


def db_benchmark(config: Config):
    if not config.do_db_bench:
        return

    print(" Performing Database benchmark ( this may take a while ) ... ")

    # clone only if dir does not exit
    if not os.path.isdir(config.substrate_repo_path):
        print(f"Cloning Substrate repository into {config.substrate_repo_path}")

        command = f"git clone https://github.com/paritytech/substrate.git {config.substrate_repo_path}".split(
            " "
        )
        result = subprocess.run(command)

        if result.returncode != 0:
            print("Failed to clone substrate repository")
            return

    read_benchmark_command = (
        "cargo run --release -p node-bench -- ::trie::read::large --json".split(" ")
    )
    write_benchmark_command = (
        "cargo run --release -p node-bench -- ::trie::write::large --json".split(" ")
    )

    read_result = subprocess.run(
        read_benchmark_command, capture_output=True, cwd=config.substrate_repo_path
    )

    if read_result.returncode != 0:
        print(f"Failed to run read DB benchmarks: {read_result.stderr}")
        return

    write_result = subprocess.run(
        write_benchmark_command, capture_output=True, cwd=config.substrate_repo_path
    )

    if write_result.returncode != 0:
        print(f"Failed to run read DB benchmarks: {write_result.stderr}")
        return

    read_result = json.loads(read_result.stdout)
    write_result = json.loads(write_result.stdout)
    return read_result, write_result


def display_db_benchmark_results(results):
    if not results:
        return

    print("Database benchmark results:\n\n")
    print(f"{'Name':^75}|{'Raw average(ns)':^26}|{'Average(ns)':^21}|")

    for oper in results:
        for result in oper:
            print(
                f"{result['name']:<75}| {result['raw_average']:^25}| {result['average']:^20}|"
            )


def please_do():
    print("HydraDX Node Performance check ... ")

    config = Config(
        do_db_bench=args.db_benchmark,
        substrate_repo_path="./substrate" if not args.sb_repo else args.sb_repo,
        do_pallet_bench=not args.only_db_benchmark,
    )

    if config.do_pallet_bench:
        s = load_hydra_values(HYDRA_REF_VALUES_LOCATION)

        benchmarks = prepare_benchmarks(config, s)
        run_benchmarks(benchmarks)

        if [b._acceptable for b in benchmarks].count(False) == 1:
            # of ony failed - rerun it
            run_benchmarks(benchmarks, True)

        print("\nResults:\n\n")

        print(
            f"{'Pallet':^25}|{'Time comparison (µs)':^27}|{'diff*':^15}|{'': ^12}| {'Rerun': ^10}"
        )

        for bench in benchmarks:
            show_pallet_result(bench)

    db_results = db_benchmark(config)

    display_db_benchmark_results(db_results)

    print("\nNotes:")
    print(
        "* - diff means the difference between HydraDX reference total time and total benchmark time of current machine"
    )
    print(
        f"* - If diff >= 0 - ( {DIFF_MARGIN}% of ref value) -> performance is same or better"
    )
    print(
        f"* - If diff < 0 - ( {DIFF_MARGIN}% of ref value) -> performance is worse and might not be suitable to run HydraDX node ( You may ask HydraDX devs for further clarifications)"
    )


if __name__ == "__main__":
    please_do()
