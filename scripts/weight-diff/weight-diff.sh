#!/usr/bin/env bash
# Render a markdown report of weight changes between two directories
# of auto-generated Substrate weight files.
#
# Usage:
#   weight-diff.sh --old <dir> --new <dir> [--threshold PCT] [--top N]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PARSER="$SCRIPT_DIR/parse_weights.awk"

OLD_DIR=""; NEW_DIR=""
THRESHOLD=10
TOP_N=10

while [[ $# -gt 0 ]]; do
	case "$1" in
		--old)       OLD_DIR="$2"; shift 2 ;;
		--new)       NEW_DIR="$2"; shift 2 ;;
		--threshold) THRESHOLD="$2"; shift 2 ;;
		--top)       TOP_N="$2"; shift 2 ;;
		-h|--help)
			grep -E '^# ' "$0" | sed 's/^# \{0,1\}//'
			exit 0 ;;
		*) echo "Unknown arg: $1" >&2; exit 2 ;;
	esac
done

[[ -z "$OLD_DIR" || -z "$NEW_DIR" ]] && { echo "Usage: $0 --old DIR --new DIR [--threshold PCT] [--top N]" >&2; exit 2; }
[[ -d "$OLD_DIR" ]] || { echo "Not a dir: $OLD_DIR" >&2; exit 2; }
[[ -d "$NEW_DIR" ]] || { echo "Not a dir: $NEW_DIR" >&2; exit 2; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

CHANGED_TSV="$WORK/changed.tsv"   # extrinsics in modified files
NEW_FILES_TSV="$WORK/new_files.tsv"
REMOVED_FILES_TSV="$WORK/removed_files.tsv"
: > "$CHANGED_TSV"; : > "$NEW_FILES_TSV"; : > "$REMOVED_FILES_TSV"

CHANGED_FILES=0

# Union of basenames in both dirs
{
	[[ -d "$OLD_DIR" ]] && find "$OLD_DIR" -maxdepth 1 -name '*.rs' -type f -exec basename {} \;
	[[ -d "$NEW_DIR" ]] && find "$NEW_DIR" -maxdepth 1 -name '*.rs' -type f -exec basename {} \;
} | sort -u > "$WORK/files"

while IFS= read -r f; do
	[[ "$f" == "mod.rs" ]] && continue
	pallet="${f%.rs}"
	old="$OLD_DIR/$f"
	new="$NEW_DIR/$f"

	if [[ -f "$old" && -f "$new" ]]; then
		cmp -s "$old" "$new" && continue
		CHANGED_FILES=$((CHANGED_FILES+1))
		awk -f "$PARSER" "$old" > "$WORK/_old.tsv"
		awk -f "$PARSER" "$new" > "$WORK/_new.tsv"
		awk -v pallet="$pallet" -F'\t' 'BEGIN{OFS="\t"}
			FNR==NR { old[$1]=$0; next }
			{
				if ($1 in old) {
					split(old[$1], o, "\t")
					print "changed", pallet, $1, o[2], $2, o[3], $3, o[4], $4, o[5], $5
					seen[$1]=1
				} else {
					print "new_fn", pallet, $1, 0, $2, 0, $3, 0, $4, 0, $5
				}
			}
			END {
				for (fn in old) if (!(fn in seen)) {
					split(old[fn], o, "\t")
					print "removed_fn", pallet, fn, o[2], 0, o[3], 0, o[4], 0, o[5], 0
				}
			}
		' "$WORK/_old.tsv" "$WORK/_new.tsv" >> "$CHANGED_TSV"
	elif [[ -f "$new" ]]; then
		CHANGED_FILES=$((CHANGED_FILES+1))
		awk -f "$PARSER" "$new" | awk -v pallet="$pallet" -F'\t' 'BEGIN{OFS="\t"}{
			print "new_file", pallet, $1, 0, $2, 0, $3, 0, $4, 0, $5
		}' >> "$NEW_FILES_TSV"
	elif [[ -f "$old" ]]; then
		CHANGED_FILES=$((CHANGED_FILES+1))
		awk -f "$PARSER" "$old" | awk -v pallet="$pallet" -F'\t' 'BEGIN{OFS="\t"}{
			print "removed_file", pallet, $1, $2, 0, $3, 0, $4, 0, $5, 0
		}' >> "$REMOVED_FILES_TSV"
	fi
done < "$WORK/files"

# Append signed ref_pct, proof_pct columns (12, 13). Only finite when old > 0.
# Columns after: 1 status, 2 pallet, 3 fn, 4 ref_old, 5 ref_new, 6 proof_old, 7 proof_new,
#                8 reads_old, 9 reads_new, 10 writes_old, 11 writes_new, 12 ref_pct, 13 proof_pct
ROWS="$WORK/rows.tsv"
awk -F'\t' 'BEGIN{OFS="\t"} {
	ref_old=$4+0; ref_new=$5+0; proof_old=$6+0; proof_new=$7+0
	ref_pct   = (ref_old   > 0) ? (ref_new   - ref_old)   * 100.0 / ref_old   : (ref_new   > 0 ? 100.0 : 0)
	proof_pct = (proof_old > 0) ? (proof_new - proof_old) * 100.0 / proof_old : (proof_new > 0 ? 100.0 : 0)
	print $0, ref_pct, proof_pct
}' "$CHANGED_TSV" > "$ROWS"

# Counts — only count extrinsics with an actual metric delta
TOTAL_CHANGED=$(awk -F'\t' '$1=="changed" && ($4!=$5 || $6!=$7 || $8!=$9 || $10!=$11)' "$ROWS" | wc -l | tr -d ' ')
TOTAL_NEW_FNS=$(grep -c '^new_fn' "$ROWS" || true)
TOTAL_REMOVED_FNS=$(grep -c '^removed_fn' "$ROWS" || true)
TOTAL_NEW_FILE_FNS=$(wc -l < "$NEW_FILES_TSV" | tr -d ' ')
TOTAL_REMOVED_FILE_FNS=$(wc -l < "$REMOVED_FILES_TSV" | tr -d ' ')

# Big changes
BIG_REF=$(awk -F'\t' -v t="$THRESHOLD" '$1=="changed" && ($12+0 >= t || $12+0 <= -t)' "$ROWS" | wc -l | tr -d ' ')
BIG_PROOF=$(awk -F'\t' -v t="$THRESHOLD" '$1=="changed" && ($13+0 >= t || $13+0 <= -t)' "$ROWS" | wc -l | tr -d ' ')

# Formatters --------------------------------------------------------------
fmt_num() {
	# Pretty-print a non-negative integer with K/M/B suffix and one decimal.
	awk -v n="$1" 'BEGIN{
		x = n + 0
		if (x == 0) { print "0"; exit }
		if (x >= 1e9) printf "%.1fB\n", x/1e9
		else if (x >= 1e6) printf "%.1fM\n", x/1e6
		else if (x >= 1e3) printf "%.1fK\n", x/1e3
		else printf "%d\n", x
	}'
}
fmt_pct() {
	# Bold the percentage if |p| >= threshold, prepend a warning emoji when so.
	awk -v p="$1" -v t="$THRESHOLD" 'BEGIN{
		a = (p < 0 ? -p : p)
		if (a >= t) printf "⚠️ **%+.1f%%**\n", p
		else        printf "%+.1f%%\n", p
	}'
}

print_row_ref_change() {
	# args: pallet fn ref_old ref_new ref_pct
	local pallet="$1" fn="$2" ro="$3" rn="$4" rp="$5"
	printf "| %s | %s | %s (%s → %s) |\n" \
		"$pallet" "\`$fn\`" "$(fmt_pct "$rp")" "$(fmt_num "$ro")" "$(fmt_num "$rn")"
}
print_row_proof_change() {
	local pallet="$1" fn="$2" po="$3" pn="$4" pp="$5"
	printf "| %s | %s | %s (%s → %s) |\n" \
		"$pallet" "\`$fn\`" "$(fmt_pct "$pp")" "$po" "$pn"
}

# Output ------------------------------------------------------------------
out() { printf "%s\n" "$*"; }

# Header
TOTAL_FILES=$CHANGED_FILES
TOTAL_BIG=$((BIG_REF + BIG_PROOF))

out "## Weight Diff Report"
out ""
if [[ $TOTAL_CHANGED -eq 0 && $TOTAL_NEW_FNS -eq 0 && $TOTAL_REMOVED_FNS -eq 0 && $TOTAL_NEW_FILE_FNS -eq 0 && $TOTAL_REMOVED_FILE_FNS -eq 0 ]]; then
	out "_No weight changes detected._"
	exit 0
fi

if (( TOTAL_BIG > 0 )); then
	out "> ⚠️ **${TOTAL_BIG} extrinsic(s) exceed ±${THRESHOLD}% threshold** (ref_time: $BIG_REF, proof_size: $BIG_PROOF)"
	out ""
fi

ALL_NEW=$((TOTAL_NEW_FNS + TOTAL_NEW_FILE_FNS))
ALL_REMOVED=$((TOTAL_REMOVED_FNS + TOTAL_REMOVED_FILE_FNS))
out "**${TOTAL_CHANGED} extrinsic(s) changed** across **${TOTAL_FILES} file(s)**. New: ${ALL_NEW}. Removed: ${ALL_REMOVED}."
out ""

# Biggest increases (RefTime)
INC=$(awk -F'\t' '$1=="changed" && $12+0 > 0' "$ROWS" | sort -t$'\t' -k12,12 -nr | head -n "$TOP_N")
if [[ -n "$INC" ]]; then
	out "### Biggest Increases (RefTime)"
	out ""
	out "| Pallet | Extrinsic | Change |"
	out "|---|---|---|"
	while IFS=$'\t' read -r status pallet fn ref_old ref_new proof_old proof_new reads_old reads_new writes_old writes_new ref_pct proof_pct; do
		print_row_ref_change "$pallet" "$fn" "$ref_old" "$ref_new" "$ref_pct"
	done <<< "$INC"
	out ""
fi

# Biggest decreases (RefTime)
DEC=$(awk -F'\t' '$1=="changed" && $12+0 < 0' "$ROWS" | sort -t$'\t' -k12,12 -n | head -n "$TOP_N")
if [[ -n "$DEC" ]]; then
	out "### Biggest Decreases (RefTime)"
	out ""
	out "| Pallet | Extrinsic | Change |"
	out "|---|---|---|"
	while IFS=$'\t' read -r status pallet fn ref_old ref_new proof_old proof_new reads_old reads_new writes_old writes_new ref_pct proof_pct; do
		print_row_ref_change "$pallet" "$fn" "$ref_old" "$ref_new" "$ref_pct"
	done <<< "$DEC"
	out ""
fi

# Biggest Proof Size changes (signed, by absolute %)
PROOF=$(awk -F'\t' 'BEGIN{OFS="\t"} $1=="changed" && $13+0 != 0 { a = ($13<0 ? -$13 : $13); print a, $0 }' "$ROWS" | sort -t$'\t' -k1,1 -nr | cut -f2- | head -n "$TOP_N")
if [[ -n "$PROOF" ]]; then
	out "### Biggest Changes (Proof Size)"
	out ""
	out "| Pallet | Extrinsic | Change |"
	out "|---|---|---|"
	while IFS=$'\t' read -r status pallet fn ref_old ref_new proof_old proof_new reads_old reads_new writes_old writes_new ref_pct proof_pct; do
		print_row_proof_change "$pallet" "$fn" "$proof_old" "$proof_new" "$proof_pct"
	done <<< "$PROOF"
	out ""
fi

# Reads/Writes deltas
RW=$(awk -F'\t' '$1=="changed" && ($8 != $9 || $10 != $11)' "$ROWS")
if [[ -n "$RW" ]]; then
	out "### DB Reads / Writes Changes"
	out ""
	out "| Pallet | Extrinsic | Reads | Writes |"
	out "|---|---|---|---|"
	while IFS=$'\t' read -r status pallet fn ref_old ref_new proof_old proof_new reads_old reads_new writes_old writes_new ref_pct proof_pct; do
		rcell=""
		if [[ "$reads_old" != "$reads_new" ]]; then
			delta=$((reads_new - reads_old))
			[[ $delta -gt 0 ]] && d="+${delta}" || d="${delta}"
			rcell="${reads_old} → ${reads_new} (**${d}**)"
		else
			rcell="${reads_new}"
		fi
		wcell=""
		if [[ "$writes_old" != "$writes_new" ]]; then
			delta=$((writes_new - writes_old))
			[[ $delta -gt 0 ]] && d="+${delta}" || d="${delta}"
			wcell="${writes_old} → ${writes_new} (**${d}**)"
		else
			wcell="${writes_new}"
		fi
		printf "| %s | \`%s\` | %s | %s |\n" "$pallet" "$fn" "$rcell" "$wcell"
	done <<< "$RW"
	out ""
fi

# New fns (in modified files) + new files — sort by pallet then fn for stable ordering
NEW_ROWS=$( { awk -F'\t' '$1=="new_fn"' "$ROWS"; cat "$NEW_FILES_TSV" 2>/dev/null; } | sort -s -t$'\t' -k2,2 || true)
if [[ -n "$NEW_ROWS" ]]; then
	count=$(printf "%s\n" "$NEW_ROWS" | grep -c . || true)
	out "<details><summary>New extrinsics (${count})</summary>"
	out ""
	out "| Pallet | Extrinsic | RefTime | Proof | Reads | Writes |"
	out "|---|---|---|---|---|---|"
	while IFS=$'\t' read -r status pallet fn ref_old ref_new proof_old proof_new reads_old reads_new writes_old writes_new _rest; do
		[[ -z "$status" ]] && continue
		printf "| %s | \`%s\` | %s | %s | %s | %s |\n" \
			"$pallet" "$fn" "$(fmt_num "$ref_new")" "$proof_new" "$reads_new" "$writes_new"
	done <<< "$NEW_ROWS"
	out ""
	out "</details>"
	out ""
fi

# Removed fns + removed files — sort by pallet then fn
RM_ROWS=$( { awk -F'\t' '$1=="removed_fn"' "$ROWS"; cat "$REMOVED_FILES_TSV" 2>/dev/null; } | sort -s -t$'\t' -k2,2 || true)
if [[ -n "$RM_ROWS" ]]; then
	count=$(printf "%s\n" "$RM_ROWS" | grep -c . || true)
	out "<details><summary>Removed extrinsics (${count})</summary>"
	out ""
	out "| Pallet | Extrinsic | RefTime | Proof | Reads | Writes |"
	out "|---|---|---|---|---|---|"
	while IFS=$'\t' read -r status pallet fn ref_old ref_new proof_old proof_new reads_old reads_new writes_old writes_new _rest; do
		[[ -z "$status" ]] && continue
		printf "| %s | \`%s\` | %s | %s | %s | %s |\n" \
			"$pallet" "$fn" "$(fmt_num "$ref_old")" "$proof_old" "$reads_old" "$writes_old"
	done <<< "$RM_ROWS"
	out ""
	out "</details>"
	out ""
fi

out "---"
out "_Threshold: ±${THRESHOLD}%. Top ${TOP_N} per section. Base \`Weight::from_parts(ref_time, proof_size)\` compared; per-unit components ignored._"
