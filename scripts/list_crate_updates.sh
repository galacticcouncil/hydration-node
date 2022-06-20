#!/usr/bin/env bash

# this script is used in github workflow to print an overview of crates and its version

set -ex

RUNTIME_CRATE="hydradx-runtime"
PROJECT_PATH=$(cargo locate-project --workspace --message-format plain)
PROJECT_PATH=${PROJECT_PATH%Cargo.toml}

ACTUAL_COMMIT=$(git rev-parse HEAD)
BASE_COMMIT=$(git rev-parse origin/${GITHUB_BASE_REF:=master})

git fetch --quiet --depth 1 origin "$BASE_COMMIT"
git checkout --quiet "$BASE_COMMIT"

# get list of local crates and remove empty lines from the output
IFS=$'\n' read -r -d '' -a CRATE_ARR_MASTER < <( cargo tree --edges normal --depth 0 | sed -r '/^\s*$/d' && printf '\0' )

CRATE_VERSION_MASTER_ARR=()
CRATE_PATH_MASTER_ARR=()

for crate in "${CRATE_ARR_MASTER[@]}"; do
    VARS=( $crate )
    CRATE_VERSION_MASTER_ARR+=( ${VARS[1]} )
    CRATE_PATH_MASTER=$( echo ${VARS[2]} | sed 's/^.\(.*\).$/\1/' )
    CRATE_PATH_MASTER=${CRATE_PATH_MASTER#$PROJECT_PATH}
    CRATE_PATH_MASTER_ARR+=("$CRATE_PATH_MASTER")
done

# test that the same runtime versions are used
IFS=$'\n' read -r -d '' -a RUNTIMES_MASTER < <( printf '%s\n' "${CRATE_ARR_MASTER[@]}" | grep $RUNTIME_CRATE && printf '\0' )

RUNTIME_SPEC_VERSIONS_MASTER=()
RUNTIME_NAMES_MASTER=()
for vars in "${RUNTIMES_MASTER[@]}"; do
#    IFS=$' '
    VARS=( $vars )
    RUNTIME_NAMES_MASTER+=("${VARS[0]}")
    CRATE_PATH=$( echo ${VARS[2]} | sed 's/^.\(.*\).$/\1/' )
    RUNTIME_SPEC_VERSIONS_MASTER+=($(grep -rI "spec_version:" "$CRATE_PATH" | grep -o -E "[0-9]+"))
#    IFS=$'\n'
done

git checkout -f --quiet "$ACTUAL_COMMIT"

MODIFIED_FILES=($(git diff --name-only "$ACTUAL_COMMIT" "$BASE_COMMIT"))

# get list of local crates and remove empty lines from the output
IFS=$'\n' read -r -d '' -a CRATE_ARR< <( cargo tree --edges normal --depth 0 | sed -r '/^\s*$/d' && printf '\0' )

CRATE_NAME_ARR=()
CRATE_VERSION_ARR=()
CRATE_PATH_ARR=()

for crate in "${CRATE_ARR[@]}"; do
    VARS=( $crate )
    CRATE_NAME_ARR+=("${VARS[0]}")
    CRATE_VERSION_ARR+=("${VARS[1]}")
    CRATE_PATH=$( echo ${VARS[2]} | sed 's/^.\(.*\).$/\1/' )
    CRATE_PATH=${CRATE_PATH#$PROJECT_PATH}
    CRATE_PATH_ARR+=("$CRATE_PATH")
done

# sort the list by length. This step is to prioritize nested crates
IFS=$'\n' GLOBIGNORE='*' CRATE_PATH_ARR_SORTED=($(printf '%s\n' "${CRATE_PATH_ARR[@]}" | awk '{ print length($0) " " $0; }' | sort -r -n | cut -d ' ' -f 2-))

MODIFIED_CRATES_ARR=()
for modified_file in "${MODIFIED_FILES[@]}"; do
    for CRATE_PATH in "${CRATE_PATH_ARR_SORTED[@]}"; do
        if [[ $modified_file =~ ^$CRATE_PATH ]]; then
            MODIFIED_CRATES_ARR+=("$CRATE_PATH")
            continue
        fi
    done
done

# remove duplicates
MODIFIED_CRATES_ARR=( $(printf '%s\n' "${MODIFIED_CRATES_ARR[@]}" | sort -u) )

NOT_UPDATED_VERSIONS_ARR=()
UPDATED_VERSIONS_ARR=()
DOWNGRADED_VERSIONS_ARR=()
NEW_VERSIONS_ARR=()

for crate in "${MODIFIED_CRATES_ARR[@]}"; do
    # get index for the current revision
    CURRENT_CRATE_INDEX=""
    for i in "${!CRATE_PATH_ARR[@]}"; do
        if [[ "${CRATE_PATH_ARR[$i]}" = "${crate}" ]]; then
          CURRENT_CRATE_INDEX=$i
        fi
    done

    CRATE_NAME=${CRATE_NAME_ARR[CURRENT_CRATE_INDEX]}
    NEW_VERSION=${CRATE_VERSION_ARR[CURRENT_CRATE_INDEX]}

    # get index for master
    MASTER_CRATE_INDEX=""
    for i in "${!CRATE_PATH_MASTER_ARR[@]}"; do
        if [[ "${CRATE_PATH_MASTER_ARR[$i]}" = "${crate}" ]]; then
          MASTER_CRATE_INDEX=$i
        fi
    done

    # crate has the same version
    if [ "$NEW_VERSION" == "${CRATE_VERSION_MASTER_ARR[MASTER_CRATE_INDEX]}" ]; then
      NOT_UPDATED_VERSIONS_ARR+=("$CRATE_NAME: $NEW_VERSION")
    # new crate
    elif [ -z "$MASTER_CRATE_INDEX" ]; then
      NEW_VERSIONS_ARR+=("$CRATE_NAME: $NEW_VERSION")
    # crate has different versions
    else
      if [ "$NEW_VERSION" == "`echo -e "$NEW_VERSION\n${CRATE_VERSION_MASTER_ARR[MASTER_CRATE_INDEX]}" | sort -Vr | head -n1`" ]; then
        UPDATED_VERSIONS_ARR+=("$CRATE_NAME: ${CRATE_VERSION_MASTER_ARR[$MASTER_CRATE_INDEX]} -> $NEW_VERSION")
      else
        DOWNGRADED_VERSIONS_ARR+=("$CRATE_NAME: ${CRATE_VERSION_MASTER_ARR[$MASTER_CRATE_INDEX]} -> $NEW_VERSION")
      fi
    fi
done

# test that the same runtime versions are used
IFS=$'\n' read -r -d '' -a RUNTIMES < <( printf '%s\n' "${CRATE_ARR[@]}" | grep $RUNTIME_CRATE && printf '\0' )

RUNTIME_CARGO_VERSIONS=()
RUNTIME_SPEC_VERSIONS=()
RUNTIME_NAMES=()
HAS_RUNTIME_VERSION_CHANGED=false
for vars in "${RUNTIMES[@]}"; do
    IFS=$' '
    VARS=( $vars )
    RUNTIME_NAMES+=("${VARS[0]}")
    CRATE_PATH=$( echo ${VARS[2]} | sed 's/^.\(.*\).$/\1/' )
    RUNTIME_SPEC_VERSION=$(grep -rI "spec_version:" "$CRATE_PATH" | grep -o -E "[0-9]+")
    RUNTIME_SPEC_VERSIONS+=($RUNTIME_SPEC_VERSION)
    VERSION=( ${VARS[1]//./ } )
    RUNTIME_CARGO_VERSIONS+=($( echo "${VERSION[0]}" | grep -o -E "[0-9]+"))
    IFS=$'\n'

    for i in "${!RUNTIME_NAMES_MASTER[@]}"; do
        if [ "${RUNTIME_NAMES_MASTER[i]}" == "${VARS[0]}" ]; then
            if [ "${RUNTIME_SPEC_VERSIONS_MASTER[i]}" != "$RUNTIME_SPEC_VERSION" ]; then
                HAS_RUNTIME_VERSION_CHANGED=true
            fi
        fi
    done
done

RUNTIME_VERSION_DIFFS=()
for i in "${!RUNTIME_NAMES[@]}"; do
    if [ "${RUNTIME_CARGO_VERSIONS[i]}" != "${RUNTIME_SPEC_VERSIONS[i]}" ]; then
      RUNTIME_VERSION_DIFFS+=("${RUNTIME_NAMES[i]}: cargo and spec versions don't match.")
    fi
done

# print the results
if [ ${#NOT_UPDATED_VERSIONS_ARR[@]} -ne 0 ]; then
    echo "Crate versions that have not been updated:"
    for line in ${NOT_UPDATED_VERSIONS_ARR[@]}; do
      echo "- $line"
    done
    echo
fi

if [ ${#NEW_VERSIONS_ARR[@]} -ne 0 ]; then
    echo "New crates:"
    for line in ${NEW_VERSIONS_ARR[@]}; do
      echo "- $line"
    done
    echo
fi

if [ ${#UPDATED_VERSIONS_ARR[@]} -ne 0 ]; then
    echo "Crate versions that have been updated:"
    for line in ${UPDATED_VERSIONS_ARR[@]}; do
      echo "- $line"
    done
    echo
fi

if [ ${#DOWNGRADED_VERSIONS_ARR[@]} -ne 0 ]; then
    echo "Crate versions that have been downgraded:"
    for line in ${DOWNGRADED_VERSIONS_ARR[@]}; do
      echo "- $line"
    done
    echo
fi

if [ ${#RUNTIME_VERSION_DIFFS[@]} -ne 0 ]; then
    for line in "${RUNTIME_VERSION_DIFFS[@]}"; do
      echo "$line"
    done
    echo
fi

RUNTIME_CARGO_VERSIONS=( $(printf '%s\n' "${RUNTIME_CARGO_VERSIONS[@]}" | sort -u) )
if [ ${#RUNTIME_CARGO_VERSIONS[@]} -gt 1 ]; then
  echo "Runtime versions don't match."
  echo
fi

if [ "$HAS_RUNTIME_VERSION_CHANGED" == true ]; then
    echo "Runtime version has been increased."
else
    echo "Runtime version has not been increased."
fi

if [ ${#NOT_UPDATED_VERSIONS_ARR[@]} -eq 0 -a ${#NEW_VERSIONS_ARR[@]} -eq 0 -a ${#UPDATED_VERSIONS_ARR[@]} -eq 0 -a ${#RUNTIME_VERSION_DIFFS[@]} -eq 0 -a ${#RUNTIME_CARGO_VERSIONS} -lt 2 ]; then
  echo "No changes have been detected in the local crates."
fi
