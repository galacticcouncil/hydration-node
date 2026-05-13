#!/usr/bin/env python3
"""Render a markdown report of weight changes between two directories
of auto-generated Substrate weight files.

Usage:
    weight_diff.py --old <dir> --new <dir> [--threshold PCT]
    weight_diff.py --self-test
"""

from __future__ import annotations

import argparse
import filecmp
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Iterator

# Default flag threshold for the section/cell warnings (percent).
DEFAULT_THRESHOLD: float = 10.0


# ----------------------------------------------------------------------------
# Parser
# ----------------------------------------------------------------------------

FN_START = re.compile(r"^\s*fn ([a-zA-Z0-9_]+)\([^)]*\)\s*->\s*Weight\s*\{")
FROM_PARTS = re.compile(r"Weight::from_parts\(([0-9_]+),\s*([0-9_]+)\)")
READS = re.compile(r"\.reads\(([0-9_]+)(?:_u64)?\)")
WRITES = re.compile(r"\.writes\(([0-9_]+)(?:_u64)?\)")
FN_END = re.compile(r"^\s*\}\s*$")


def _to_int(s: str) -> int:
    return int(s.replace("_", ""))


def parse_weights(text: str) -> Iterator[tuple[str, int, int, int, int]]:
    """Yield (fn_name, ref_time, proof_size, reads, writes) for each fn.

    Captures only the base `Weight::from_parts(...)`; per-unit components
    inside `.saturating_add(Weight::from_parts(...).saturating_mul(...))`
    are ignored.
    """
    fn: str | None = None
    ref = proof = reads = writes = 0
    got_parts = False
    for line in text.splitlines():
        if fn is None:
            m = FN_START.match(line)
            if m:
                fn = m.group(1)
                ref = proof = reads = writes = 0
                got_parts = False
            continue

        if not got_parts:
            m = FROM_PARTS.search(line)
            if m:
                ref = _to_int(m.group(1))
                proof = _to_int(m.group(2))
                got_parts = True
                continue

        m = READS.search(line)
        if m:
            reads = _to_int(m.group(1))
            continue

        m = WRITES.search(line)
        if m:
            writes = _to_int(m.group(1))
            continue

        if FN_END.match(line):
            yield (fn, ref, proof, reads, writes)
            fn = None


# ----------------------------------------------------------------------------
# Model
# ----------------------------------------------------------------------------

CHANGED = "changed"
NEW_FN = "new_fn"
REMOVED_FN = "removed_fn"
NEW_FILE = "new_file"
REMOVED_FILE = "removed_file"


@dataclass
class Row:
    status: str
    pallet: str
    fn: str
    ref_old: int = 0
    ref_new: int = 0
    proof_old: int = 0
    proof_new: int = 0
    reads_old: int = 0
    reads_new: int = 0
    writes_old: int = 0
    writes_new: int = 0

    @property
    def ref_pct(self) -> float:
        if self.ref_old > 0:
            return (self.ref_new - self.ref_old) * 100.0 / self.ref_old
        return 100.0 if self.ref_new > 0 else 0.0

    @property
    def proof_pct(self) -> float:
        if self.proof_old > 0:
            return (self.proof_new - self.proof_old) * 100.0 / self.proof_old
        return 100.0 if self.proof_new > 0 else 0.0

    @property
    def has_metric_delta(self) -> bool:
        return (
            self.ref_old != self.ref_new
            or self.proof_old != self.proof_new
            or self.reads_old != self.reads_new
            or self.writes_old != self.writes_new
        )


# ----------------------------------------------------------------------------
# Diff
# ----------------------------------------------------------------------------


def diff_dirs(old_dir: Path, new_dir: Path) -> tuple[list[Row], int]:
    rows: list[Row] = []
    changed_files = 0

    old_files = {p.name for p in old_dir.glob("*.rs") if p.name != "mod.rs"}
    new_files = {p.name for p in new_dir.glob("*.rs") if p.name != "mod.rs"}
    all_files = sorted(old_files | new_files)

    for fname in all_files:
        pallet = fname[:-3]  # strip .rs
        old_path = old_dir / fname
        new_path = new_dir / fname

        in_old = old_path.exists()
        in_new = new_path.exists()

        if in_old and in_new:
            if filecmp.cmp(old_path, new_path, shallow=False):
                continue
            changed_files += 1
            old_map = {fn: (rt, ps, r, w) for fn, rt, ps, r, w in parse_weights(old_path.read_text())}
            new_map = {fn: (rt, ps, r, w) for fn, rt, ps, r, w in parse_weights(new_path.read_text())}
            for fn, (rt, ps, r, w) in new_map.items():
                if fn in old_map:
                    o_rt, o_ps, o_r, o_w = old_map[fn]
                    rows.append(Row(CHANGED, pallet, fn, o_rt, rt, o_ps, ps, o_r, r, o_w, w))
                else:
                    rows.append(Row(NEW_FN, pallet, fn, 0, rt, 0, ps, 0, r, 0, w))
            for fn, (rt, ps, r, w) in old_map.items():
                if fn not in new_map:
                    rows.append(Row(REMOVED_FN, pallet, fn, rt, 0, ps, 0, r, 0, w, 0))
        elif in_new:
            changed_files += 1
            for fn, rt, ps, r, w in parse_weights(new_path.read_text()):
                rows.append(Row(NEW_FILE, pallet, fn, 0, rt, 0, ps, 0, r, 0, w))
        elif in_old:
            changed_files += 1
            for fn, rt, ps, r, w in parse_weights(old_path.read_text()):
                rows.append(Row(REMOVED_FILE, pallet, fn, rt, 0, ps, 0, r, 0, w, 0))

    return rows, changed_files


# ----------------------------------------------------------------------------
# Render
# ----------------------------------------------------------------------------


def fmt_num(n: int) -> str:
    if n == 0:
        return "0"
    x = float(n)
    if x >= 1e9:
        return f"{x / 1e9:.1f}B"
    if x >= 1e6:
        return f"{x / 1e6:.1f}M"
    if x >= 1e3:
        return f"{x / 1e3:.1f}K"
    return str(n)


def fmt_pct(p: float, threshold: float) -> str:
    if abs(p) >= threshold:
        return f"⚠️ **{p:+.1f}%**"
    return f"{p:+.1f}%"


def _cell_ref(r: Row, threshold: float) -> str:
    if r.ref_old == r.ref_new:
        return "—"
    return f"{fmt_pct(r.ref_pct, threshold)} ({fmt_num(r.ref_old)} → {fmt_num(r.ref_new)})"


def _cell_proof(r: Row, threshold: float) -> str:
    if r.proof_old == r.proof_new:
        return "—"
    return f"{fmt_pct(r.proof_pct, threshold)} ({r.proof_old} → {r.proof_new})"


def _cell_reads(r: Row) -> str:
    if r.reads_old == r.reads_new:
        return "—"
    d = r.reads_new - r.reads_old
    return f"{r.reads_old} → {r.reads_new} (**{d:+d}**)"


def _cell_writes(r: Row) -> str:
    if r.writes_old == r.writes_new:
        return "—"
    d = r.writes_new - r.writes_old
    return f"{r.writes_old} → {r.writes_new} (**{d:+d}**)"


def render(rows: list[Row], changed_files: int, threshold: float) -> str:
    out: list[str] = []
    out.append("## Weight Diff Report")
    out.append("")

    changed = [r for r in rows if r.status == CHANGED and r.has_metric_delta]
    new_fns = [r for r in rows if r.status in (NEW_FN, NEW_FILE)]
    removed_fns = [r for r in rows if r.status in (REMOVED_FN, REMOVED_FILE)]

    if not changed and not new_fns and not removed_fns:
        out.append("_No weight changes detected._")
        return "\n".join(out) + "\n"

    # Group changed rows by pallet (preserving document order within each pallet).
    by_pallet: dict[str, list[Row]] = {}
    for r in changed:
        by_pallet.setdefault(r.pallet, []).append(r)

    def pallet_warns(rs: list[Row]) -> bool:
        return any(abs(r.ref_pct) >= threshold or abs(r.proof_pct) >= threshold for r in rs)

    warned = {p for p, rs in by_pallet.items() if pallet_warns(rs)}

    if warned:
        out.append(
            f"> ⚠️ **{len(warned)} pallet(s) have changes exceeding ±{threshold:g}% threshold**"
        )
        out.append("")

    out.append(
        f"**{len(changed)} extrinsic(s) changed** across **{len(by_pallet)} pallet(s)**. "
        f"New: {len(new_fns)}. Removed: {len(removed_fns)}."
    )
    out.append("")

    # Order: warned pallets first (alphabetical), then non-warned (alphabetical).
    ordered = sorted(by_pallet.keys(), key=lambda p: (p not in warned, p))

    for pallet in ordered:
        out.append(f"### {pallet}")
        out.append("")
        out.append("| Extrinsic | RefTime | Proof Size | Reads | Writes |")
        out.append("|---|---|---|---|---|")
        for r in by_pallet[pallet]:
            out.append(
                f"| `{r.fn}` | "
                f"{_cell_ref(r, threshold)} | "
                f"{_cell_proof(r, threshold)} | "
                f"{_cell_reads(r)} | "
                f"{_cell_writes(r)} |"
            )
        out.append("")

    if new_fns:
        out.append(f"<details><summary>New extrinsics ({len(new_fns)})</summary>")
        out.append("")
        out.append("| Pallet | Extrinsic | RefTime | Proof | Reads | Writes |")
        out.append("|---|---|---|---|---|---|")
        for r in new_fns:
            out.append(
                f"| {r.pallet} | `{r.fn}` | {fmt_num(r.ref_new)} | "
                f"{r.proof_new} | {r.reads_new} | {r.writes_new} |"
            )
        out.append("")
        out.append("</details>")
        out.append("")

    if removed_fns:
        out.append(f"<details><summary>Removed extrinsics ({len(removed_fns)})</summary>")
        out.append("")
        out.append("| Pallet | Extrinsic | RefTime | Proof | Reads | Writes |")
        out.append("|---|---|---|---|---|---|")
        for r in removed_fns:
            out.append(
                f"| {r.pallet} | `{r.fn}` | {fmt_num(r.ref_old)} | "
                f"{r.proof_old} | {r.reads_old} | {r.writes_old} |"
            )
        out.append("")
        out.append("</details>")
        out.append("")

    out.append("---")
    out.append(
        f"_Threshold: ±{threshold:g}%. "
        f"Base `Weight::from_parts(ref_time, proof_size)` compared; per-unit components ignored._"
    )
    return "\n".join(out) + "\n"


# ----------------------------------------------------------------------------
# Self-tests
# ----------------------------------------------------------------------------

FIXTURE_SIMPLE = """
\tfn add_collateral_asset() -> Weight {
\t\t// Proof Size summary in bytes:
\t\tWeight::from_parts(246_755_000, 28974)
\t\t\t.saturating_add(T::DbWeight::get().reads(30_u64))
\t\t\t.saturating_add(T::DbWeight::get().writes(1_u64))
\t}
"""

FIXTURE_PARAMETERIZED = """
\tfn remark(b: u32, ) -> Weight {
\t\t// Minimum execution time: 12_345_000 picoseconds.
\t\tWeight::from_parts(27_288_508, 0)
\t\t\t// Standard Error: 12
\t\t\t.saturating_add(Weight::from_parts(345, 0).saturating_mul(b.into()))
\t}
"""

FIXTURE_MULTI = """
\tfn one() -> Weight {
\t\tWeight::from_parts(100_000, 0)
\t\t\t.saturating_add(T::DbWeight::get().reads(1_u64))
\t}
\tfn two(n: u32, ) -> Weight {
\t\tWeight::from_parts(200_000, 1024)
\t\t\t.saturating_add(Weight::from_parts(50, 0).saturating_mul(n.into()))
\t\t\t.saturating_add(T::DbWeight::get().reads(3_u64))
\t\t\t.saturating_add(T::DbWeight::get().writes(2_u64))
\t}
"""


def self_test() -> int:
    failures: list[str] = []

    def check(label: str, got, want):
        if got != want:
            failures.append(f"{label}: got {got!r}, want {want!r}")

    # parse_weights — simple
    rows = list(parse_weights(FIXTURE_SIMPLE))
    check("simple count", len(rows), 1)
    check("simple row", rows[0], ("add_collateral_asset", 246_755_000, 28974, 30, 1))

    # parse_weights — parameterized fn picks BASE from_parts
    rows = list(parse_weights(FIXTURE_PARAMETERIZED))
    check("param count", len(rows), 1)
    check("param row (base, not per-unit)", rows[0], ("remark", 27_288_508, 0, 0, 0))

    # parse_weights — multi-fn
    rows = list(parse_weights(FIXTURE_MULTI))
    check("multi count", len(rows), 2)
    check("multi row 0", rows[0], ("one", 100_000, 0, 1, 0))
    check("multi row 1", rows[1], ("two", 200_000, 1024, 3, 2))

    # Row.has_metric_delta — reads-only change
    r = Row(CHANGED, "p", "f", 100, 100, 50, 50, 1, 2, 1, 1)
    check("reads-only delta detected", r.has_metric_delta, True)
    check("reads-only ref_pct=0", r.ref_pct, 0.0)
    check("reads-only proof_pct=0", r.proof_pct, 0.0)

    # Row.ref_pct
    r = Row(CHANGED, "p", "f", 100, 121, 0, 0, 0, 0, 0, 0)
    check("ref_pct +21", round(r.ref_pct, 1), 21.0)

    # Row.proof_pct with old=0 and new>0 -> +100
    r = Row(CHANGED, "p", "f", 100, 100, 0, 1024, 0, 0, 0, 0)
    check("proof_pct (0 -> N)", r.proof_pct, 100.0)

    # fmt_num
    check("fmt_num 0", fmt_num(0), "0")
    check("fmt_num 999", fmt_num(999), "999")
    check("fmt_num 1500", fmt_num(1500), "1.5K")
    check("fmt_num 246_755_000", fmt_num(246_755_000), "246.8M")
    check("fmt_num 1_747_094_800", fmt_num(1_747_094_800), "1.7B")

    # fmt_pct — below threshold no bold, above threshold bolded
    check("fmt_pct 3.0 t=10", fmt_pct(3.0, 10), "+3.0%")
    check("fmt_pct -15.0 t=10", fmt_pct(-15.0, 10), "⚠️ **-15.0%**")
    check("fmt_pct +120.0 t=10", fmt_pct(120.0, 10), "⚠️ **+120.0%**")

    if failures:
        print("SELF-TEST FAILED:")
        for f in failures:
            print(" -", f)
        return 1
    print(f"SELF-TEST OK ({22} assertions)")
    return 0


# ----------------------------------------------------------------------------
# CLI
# ----------------------------------------------------------------------------


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description="Diff Substrate weight files between two dirs.")
    p.add_argument("--old", type=Path)
    p.add_argument("--new", type=Path)
    p.add_argument("--threshold", type=float, default=DEFAULT_THRESHOLD)
    p.add_argument("--self-test", action="store_true")
    args = p.parse_args(argv)

    if args.self_test:
        return self_test()

    if not args.old or not args.new:
        p.error("--old and --new are required (or use --self-test)")
    if not args.old.is_dir():
        p.error(f"not a dir: {args.old}")
    if not args.new.is_dir():
        p.error(f"not a dir: {args.new}")

    rows, changed_files = diff_dirs(args.old, args.new)
    sys.stdout.write(render(rows, changed_files, args.threshold))
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
