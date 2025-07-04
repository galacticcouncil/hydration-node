#!/usr/bin/env bash
# Created by Moonbeam/Purestake Developers. Shamelessly copied from Moonbeam's benchmarking script
# Original repository: https://github.com/moonbeam-foundation/moonbeam

# This script can be used for running HydraDX's benchmarks.
#
# The hydradx binary is required to be compiled with --features=runtime-benchmarks
# in release mode.

set -e          # Exit immediately if a command exits with non-zero status
set -o pipefail # Ensure pipeline commands also trigger failures

BINARY="./target/release/hydradx"
OUTPUT="runtime/hydradx/src/weights/"
DEFAULT_WEIGHT_TEMPLATE="scripts/pallet-weight-template.hbs"
STEPS=50
REPEAT=20

function help {
    echo "USAGE:"
    echo "  ${0} [<pallet> <benchmark>] [--check] [--all] [--bin <path>]"
    echo ""
    echo "EXAMPLES:"
    echo "  ${0}                       " "list all benchmarks and provide a selection to choose from"
    echo "  ${0} --check               " "list all benchmarks and provide a selection to choose from, runs in 'check' mode (reduced steps and repetitions)"
    echo "  ${0} foo bar               " "run a benchmark for pallet 'foo' and benchmark 'bar'"
    echo "  ${0} foo bar --check       " "run a benchmark for pallet 'foo' and benchmark 'bar' in 'check' mode (reduced steps and repetitions)"
    echo "  ${0} --all         " "run a benchmark for all pallets (EXCEPT xcm)"
    echo "  ${0} --all --check " "run a benchmark for all pallets (EXCEPT xcm) in 'check' mode (reduced steps and repetitions)"
    echo "  ${0} --bin <path>  " "specify a path to the benchmark cli binary"
}

function choose_and_bench {
    while read benchmark; do
        options+=("$benchmark")
    done < <(${BINARY} benchmark pallet --list | sed 1d)

    options+=('EXIT')

    select opt in "${options[@]}"; do
        IFS=', ' read -ra parts <<< "${opt}"
        [[ "${opt}" == 'EXIT' ]] && exit 0

        bench "${parts[0]}" "${parts[1]}" "${1}"
        break
    done
}

function bench {
    if [[ ! -f "${BINARY}" ]]; then
        echo "binary '${BINARY}' does not exist."
        echo "ensure that the hydradx binary is compiled with '--features=runtime-benchmarks' and in release mode."
        exit 1
    fi

    local output_file=${4:-weights.rs}
    echo "benchmarking '${1}::${2}' --check=${3}, writing results to '${output_file}'"

    # Check enabled
    if [[ "${3}" -eq 1 ]]; then
        STEPS=16
        REPEAT=1
    fi

    local weight_template="$DEFAULT_WEIGHT_TEMPLATE"
    if [[ -n "${5}" ]]; then
        weight_template="${5}"
    fi

    WASMTIME_BACKTRACE_DETAILS=1 ${BINARY} benchmark pallet \
        --wasm-execution=compiled \
        --pallet "${1}" \
        --extrinsic "${2}" \
        --heap-pages 4096 \
        --steps "${STEPS}" \
        --repeat "${REPEAT}" \
        --template "${weight_template}" \
        --output "${output_file}" \
        --quiet
}

CHECK=0
ALL=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --bin)
            shift
            BINARY="$1"
            ;;
        --check)
            CHECK=1
            ;;
        --all)
            ALL=1
            ;;
        --help)
            help
            exit 0
            ;;
        *)
            ARGS+=("$1")
            ;;
    esac
    shift
done

if [[ "${ALL}" -eq 1 ]]; then
    mkdir -p "$OUTPUT"
    # FIXME: This is a temporary solution to handle XCM benchmarks correctly
    BENCH_EXCLUDE=("pallet_xcm_benchmarks::fungible" "pallet_xcm_benchmarks::generic" "pallet_scheduler")

    XCM_ADD=(
      "pallet_xcm_benchmarks::fungible withdraw_asset,transfer_asset,transfer_reserve_asset,reserve_asset_deposited,initiate_reserve_withdraw,receive_teleported_asset,deposit_asset,deposit_reserve_asset pallet_xcm_benchmarks_fungible.rs"
      "pallet_xcm_benchmarks::generic report_holding,buy_execution,query_response,transact,refund_surplus,set_error_handler,set_appendix,clear_error,descend_origin,clear_origin,report_error,claim_asset,trap,subscribe_version,unsubscribe_version,initiate_reserve_withdraw,burn_asset,expect_asset,expect_origin,expect_error,expect_transact_status,query_pallet,expect_pallet,report_transact_status,clear_transact_status,set_topic,clear_topic,set_fees_mode,unpaid_execution,exchange_asset pallet_xcm_benchmarks_generic.rs"
    )
    XCM_OUTPUT="${OUTPUT}xcm/"
    mkdir -p "$XCM_OUTPUT"

    # First, process the XCM_ADD array to run those specific benchmarks
    for xcm_entry in "${XCM_ADD[@]}"; do
      # Convert string to array
      eval "entry=($xcm_entry)"
      pallet="${entry[0]}"
      extrinsics="${entry[1]}"
      output_file="${entry[2]}"

      bench "$pallet" "$extrinsics" "$CHECK" "${XCM_OUTPUT}${output_file}" "scripts/xcm-weight-template.hbs"
    done

    # Then process regular pallets (excluding `BENCH_EXCLUDE` ones)
    options=()
    while read benchmark; do
      skip=0
      for xcm_item in "${BENCH_EXCLUDE[@]}"; do
        if [[ "$benchmark" == *"$xcm_item"* ]]; then
          skip=1
          break
        fi
      done

      if [[ $skip -eq 0 ]]; then
        options+=("$benchmark")
      fi
    done < <(${BINARY} benchmark pallet --list=pallets | sed 1d)

    for option in "${options[@]}"; do
      _path="${OUTPUT}${option}.rs"

      touch "${_path}" # TODO: Remove this once benchmarking-cli doesn't fail on missing files
      bench "${option}" '*' "${CHECK}" "${_path}"
    done
elif [[ ${#ARGS[@]} -ne 2 ]]; then
    choose_and_bench "${CHECK}"
else
    bench "${ARGS[0]}" "${ARGS[1]}" "${CHECK}"
fi
