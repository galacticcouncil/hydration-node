# Parse an auto-generated Substrate weight file and emit TSV records:
#   fn_name <TAB> ref_time <TAB> proof_size <TAB> reads <TAB> writes
#
# Numbers with `_` digit separators are normalised before emit.
# Captures the BASE Weight::from_parts(...) only; per-unit
# `.saturating_add(Weight::from_parts(...))` is ignored.
# POSIX awk compatible (no gawk extensions).

BEGIN { fn = ""; in_fn = 0; got_parts = 0 }

function clean(s) { gsub(/_/, "", s); sub(/^[[:space:]]+/, "", s); sub(/[[:space:]]+$/, "", s); return s + 0 }

# Match: `<indent>fn <name>(<any-args>) -> Weight {`
/^[[:space:]]*fn [a-zA-Z0-9_]+\([^)]*\) -> Weight \{/ {
    line = $0
    sub(/^[[:space:]]*fn /, "", line)
    sub(/\(.*$/, "", line)
    fn = line
    ref = 0; proof = 0; reads = 0; writes = 0
    in_fn = 1; got_parts = 0
    next
}

# First Weight::from_parts(REF_TIME, PROOF_SIZE) inside the fn body
in_fn && got_parts == 0 && /Weight::from_parts\(/ {
    line = $0
    sub(/^.*Weight::from_parts\(/, "", line)
    sub(/\).*$/, "", line)
    n = split(line, a, ",")
    if (n >= 2) {
        ref = clean(a[1])
        proof = clean(a[2])
        got_parts = 1
    }
    next
}

# Base .reads(N_u64) / .reads(N) — require a digit right after `(` so that
# per-unit reads like `.reads((17_u64).saturating_mul(...))` are skipped.
in_fn && /\.reads\([0-9]/ {
    line = $0
    sub(/^.*\.reads\(/, "", line)
    sub(/_u64.*$/, "", line)
    sub(/\).*$/, "", line)
    reads = clean(line)
    next
}

# Base .writes(N_u64) / .writes(N) — same digit-gated rule.
in_fn && /\.writes\([0-9]/ {
    line = $0
    sub(/^.*\.writes\(/, "", line)
    sub(/_u64.*$/, "", line)
    sub(/\).*$/, "", line)
    writes = clean(line)
    next
}

# Closing brace of the fn body (single `}` on its own line)
in_fn && /^[[:space:]]*\}[[:space:]]*$/ {
    if (fn != "") print fn "\t" ref "\t" proof "\t" reads "\t" writes
    fn = ""; in_fn = 0; got_parts = 0
}
