import argparse
import json
import os
import subprocess

from collections import defaultdict
from dataclasses import dataclass

HYDRA_REF_VALUES_LOCATION = ".maintain/bench-check/hydradx-bench-data.json"

DIFF_MARGIN = 10 # percent

COMMAND = [
    'cargo', 'run', '--release',
    '--features=runtime-benchmarks',
    '--manifest-path=node/Cargo.toml',
    '--',
    'benchmark',
    '--chain=dev',
    '--steps=5',
    '--repeat=20',
    '--extrinsic=*',
    '--execution=wasm',
    '--wasm-execution=compiled',
    '--heap-pages=4096',
]

PALLETS = ["amm", "exchange", "transaction_multi_payment"]


parser = argparse.ArgumentParser()
parser.add_argument('--include-db-benchmark', dest='db_benchmark', action='store_true', help='Perform Substrate Database benchmark')
parser.add_argument('--exclude-pallet-benchmark', dest='only_db_benchmark', action='store_true', help='Skip pallets benchmarks')
parser.add_argument('--substrate-repo-path', dest='sb_repo', type=str, required=False, help='Substrate repository path (cloned if not provided)')
args = parser.parse_args()

@dataclass
class Config:
    do_db_bench: bool = False
    substrate_repo_path: str = "./substrate"
    do_pallet_bench: bool = True

def load_hydra_values(filename):
    with open(filename,"r") as f:
        return json.load(f)

def process_extrinsic(data):
    for entry in data:
        if entry.startswith("Time"):
            return float(entry.split(" ")[-1])

def run_benchmarks():
    print("Running benchmarks - this may take a while...")

    results = defaultdict(dict)
    for pallet in PALLETS:
        command = COMMAND + [f"--pallet={pallet}"]

        result = subprocess.run(command, capture_output=True)

        lines = list(map(lambda  x: x.decode(),result.stdout.split(b'\n')))

        for idx, line in enumerate(lines):
            if line.startswith("Pallet:"):
                info = line.split(",")
                pallet_name = info[0].split(":")[1].strip()[1:-1]
                extrinsic = info[1].split(":")[1].strip()[1:-1]
                results[pallet_name][extrinsic] = process_extrinsic(lines[idx+1:idx+21])

    return results

def show_pallet_result(pallet, hydra_data, current_data):
    hydra = sum(list(map(lambda x:float(x), hydra_data.values())))

    c = []
    for key in hydra_data.keys():
        c.append(current_data[key])

    current = sum(list(map(lambda x:float(x), c)))

    hydra_margin = int(hydra * DIFF_MARGIN / 100)

    diff = int(hydra - current)

    note = "OK" if diff >= -hydra_margin else "FAILED"

    diff = f"{diff}"
    times = f"{hydra:.2f} vs {current:.2f}"

    print(f"{pallet:<25}| {times:^25} | {diff:^13} | {note:^10}")

def write_hydra_results(data,location):
    with open(location,'w') as f:
        f.write(json.dumps(data, indent=4))


def db_benchmark(config: Config):
    if not config.do_db_bench:
        return

    print(" Performing Database benchmark ( this may take a while ) ... ")

    # clone only if dir does not exit
    if not os.path.isdir(config.substrate_repo_path):
        print(f"Cloning Substrate repository into {config.substrate_repo_path}")

        command = f"git clone https://github.com/paritytech/substrate.git {config.substrate_repo_path}".split(" ")
        result = subprocess.run(command)

        if result.returncode != 0:
            print("Failed to clone substrate repository")
            return

    read_benchmark_command = "cargo run --release -p node-bench -- ::trie::read::large --json".split(" ")
    write_benchmark_command = "cargo run --release -p node-bench -- ::trie::write::large --json".split(" ")

    read_result = subprocess.run(read_benchmark_command, capture_output=True, cwd=config.substrate_repo_path)

    if read_result.returncode != 0:
        print(f"Failed to run read DB benchmarks: {read_result.stderr}")
        return

    write_result = subprocess.run(write_benchmark_command, capture_output=True, cwd=config.substrate_repo_path)

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
            print(f"{result['name']:<75}| {result['raw_average']:^25}| {result['average']:^20}|")


if __name__ == '__main__':
    print("HydraDX Node Performance check ... ")

    config = Config(do_db_bench=args.db_benchmark, substrate_repo_path="./substrate" if not args.sb_repo else args.sb_repo,
                    do_pallet_bench=not args.only_db_benchmark)

    if config.do_pallet_bench:
        s = load_hydra_values(HYDRA_REF_VALUES_LOCATION)
        results = run_benchmarks()

        print("\nResults:\n\n")

        print(f"{'Pallet':^25}|{'Time comparison (µs)':^27}|{'diff*':^15}|")

        for pallet, details in results.items():
            show_pallet_result(pallet, s[pallet], details)

    db_results = db_benchmark(config)

    display_db_benchmark_results(db_results)

    print("\nNotes:")
    print("* - diff means the difference between HydraDX reference total time and total benchmark time of current machine")
    print(f"* - If diff >= 0 - ( {DIFF_MARGIN}% of ref value) -> performance is same or better")
    print(f"* - If diff < 0 - ( {DIFF_MARGIN}% of ref value) -> performance is worse and might not be suitable to run HydraDX node ( You may ask HydraDX devs for further clarifications)")
