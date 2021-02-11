import json
import subprocess

from collections import defaultdict

HYDRA_REF_VALUES_LOCATION = ".maintain/bench-check/hydradx-bench-data.json"

DIFF_MARGIN = 15 # percent

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

if __name__ == '__main__':
    print("HydraDX Node Performance check ... ")
    s = load_hydra_values(HYDRA_REF_VALUES_LOCATION)

    results = run_benchmarks()

    print("\nResults:\n\n")

    print(f"{'Pallet':^25}|{'Time comparison (µs)':^27}|{'diff*':^15}|")

    for pallet, details in results.items():
        show_pallet_result(pallet, s[pallet], details)

    print("\nNotes:")
    print("* - diff means the difference between HydraDX reference total time and total benchmark time of current machine")
    print(f"* - If diff >= 0 - ( {DIFF_MARGIN} of ref value) -> performance is same or better")
    print(f"* - If diff < 0 - ( {DIFF_MARGIN} of ref value) -> performance is worse and might not be suitable to run HydraDX node ( You may ask HydraDX devs for further clarifications)")

    #write_hydra_results(results, "scripts/h.json")


