#!/usr/bin/env python3
"""Validate saved audit passes, assemble reports, and maintain opt-in campaign memory."""

import argparse
import fcntl
import json
import os
from pathlib import Path
import re
import tempfile

CONFIDENCE_THRESHOLD = 75
SEVERITIES = ("Critical", "High", "Medium", "Low")
MEMORY_SCHEMA = "cl0wdit-memory-v1"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_json(path):
    def unique_pairs(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, f"duplicate JSON field: {key}")
            result[key] = value
        return result
    with Path(path).open(encoding="utf-8") as stream:
        return json.load(stream, object_pairs_hook=unique_pairs)


def atomic_write(path, text):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    handle, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    try:
        with os.fdopen(handle, "w", encoding="utf-8") as stream:
            stream.write(text)
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def save_json(path, value):
    atomic_write(path, json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def text_field(value, name):
    require(isinstance(value, str) and bool(value.strip()), f"missing or invalid {name}")
    return value


def key_for(item):
    parts = [text_field(item.get(field), field) for field in ("module", "function", "bug_class")]
    require(all(not any(c in part for c in "|\n\r\t") for part in parts), "invalid finding key")
    require(re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", parts[2]), "bug_class must be kebab-case")
    return "|".join(parts)


def validate_item(item):
    require(isinstance(item, dict), "audit item must be an object")
    allowed = {"kind", "module", "function", "bug_class", "title", "description", "locations", "proof",
               "path", "fix", "verification", "confidence", "severity", "next_check", "candidate_ids",
               "agent_roles", "seam", "assumptions", "details"}
    require(not (item.keys() - allowed), f"unknown audit fields: {item.keys() - allowed}")
    key_for(item)
    require(item.get("kind") in ("FINDING", "LEAD"), "invalid finding kind")
    for field in ("title", "description", "verification"):
        text_field(item.get(field), field)
    require(isinstance(item.get("locations"), list) and item["locations"], "locations are required")
    for location in item["locations"]:
        text_field(location, "location")
    require(isinstance(item.get("candidate_ids"), list) and item["candidate_ids"], "candidate_ids are required")
    for identifier in item["candidate_ids"]:
        text_field(identifier, "candidate ID")
    if item["kind"] == "FINDING":
        for field in ("proof", "path", "fix"):
            text_field(item.get(field), field)
        require(item.get("severity") in SEVERITIES, "confirmed finding needs a severity")
        score = item.get("confidence")
        require(type(score) is int and CONFIDENCE_THRESHOLD <= score <= 100, "invalid confirmed confidence")
    else:
        require("confidence" not in item and "severity" not in item, "leads are not scored")
        text_field(item.get("next_check"), "next_check")
    for field in ("proof", "path", "fix", "next_check", "seam", "assumptions"):
        if field in item:
            text_field(item[field], field)
    if "details" in item:
        require(isinstance(item["details"], dict), "details must be an object")
        for label, detail in item["details"].items():
            text_field(label, "detail label")
            text_field(detail, "detail evidence")


def load_scan(directory):
    directory = Path(directory)
    scope = read_json(directory / "scope.json")
    require(isinstance(scope, dict), "scope must be an object")
    for field in ("scan_id", "campaign", "timestamp", "repository", "local_checkout", "target_revision",
                  "comparison", "model", "reasoning_effort", "skill_version", "skill_revision", "mode",
                  "execution"):
        text_field(scope.get(field), field)
    require(scope["execution"] in ("parallel", "sequential"), "invalid execution mode")
    require(not directory.resolve().is_relative_to(Path(scope["local_checkout"]).resolve()),
            "keep scan artifacts outside the audited checkout")
    require(type(scope.get("passes_planned")) is int and 1 <= scope["passes_planned"] <= 10, "invalid pass count")
    roles = scope.get("roles_planned")
    require(isinstance(roles, list) and roles and len(set(roles)) == len(roles), "invalid planned roles")
    for role in roles:
        text_field(role, "role")
    require(isinstance(scope.get("files"), list) and scope["files"], "reviewed file manifest is required")
    for path in scope["files"]:
        text_field(path, "reviewed path")
    require(type(scope.get("memory")) is bool, "memory must be explicitly true or false")
    passes = []
    for path in directory.glob("pass-*.json"):
        require(re.fullmatch(r"pass-[1-9][0-9]*\.json", path.name), f"invalid pass filename: {path.name}")
        run = read_json(path)
        require(isinstance(run, dict), "pass must be an object")
        number = run.get("number")
        require(type(number) is int and 1 <= number <= scope["passes_planned"], "invalid pass number")
        require(path.name == f"pass-{number}.json", "pass filename and number differ")
        require(run.get("target_revision") == scope["target_revision"], "source revision changed between passes")
        text_field(run.get("model"), "pass model")
        text_field(run.get("reasoning_effort"), "pass reasoning effort")
        completed, failed = run.get("roles_completed"), run.get("roles_failed")
        require(isinstance(completed, list) and isinstance(failed, dict), "pass coverage is required")
        require(len(set(completed)) == len(completed), "duplicate completed role")
        require(not set(completed).intersection(failed) and set(completed).union(failed) == set(roles),
                "every planned role must be completed or failed")
        for reason in failed.values():
            text_field(reason, "failure reason")
        require(isinstance(run.get("items"), list), "pass items must be an array")
        require(completed or not run["items"], "a pass with no completed roles cannot claim findings")
        keys = set()
        for item in run["items"]:
            validate_item(item)
            key = key_for(item)
            require(key not in keys, f"duplicate key inside pass: {key}")
            keys.add(key)
            require(isinstance(item.get("agent_roles"), list) and item["agent_roles"], "item role attribution required")
            require(set(item["agent_roles"]) <= set(completed), "item attributed to an incomplete role")
        require(isinstance(run.get("rejected"), list), "rejected candidate dispositions are required")
        for rejected in run["rejected"]:
            text_field(rejected.get("candidate_id"), "rejected candidate ID")
            text_field(rejected.get("reason"), "rejection reason")
        candidates = run.get("candidates")
        require(isinstance(candidates, list), "raw candidate inventory is required")
        for identifier in candidates:
            text_field(identifier, "raw candidate ID")
        require(len(set(candidates)) == len(candidates), "duplicate raw candidate ID")
        dispositions = [identifier for item in run["items"] for identifier in item["candidate_ids"]]
        dispositions.extend(item["candidate_id"] for item in run["rejected"])
        require(len(set(dispositions)) == len(dispositions), "candidate has multiple dispositions")
        require(set(dispositions) == set(candidates), "candidate inventory and dispositions differ")
        passes.append(run)
    passes.sort(key=lambda run: run["number"])
    return scope, passes


def validate_ledger(ledger, scope):
    require(isinstance(ledger, dict) and ledger.get("schema") == MEMORY_SCHEMA, "invalid memory schema")
    require(ledger.get("repository") == scope["repository"] and ledger.get("campaign") == scope["campaign"],
            "ledger belongs to another repository or campaign")
    require(isinstance(ledger.get("records"), list), "invalid memory records")
    seen = set()
    for record in ledger["records"]:
        require(isinstance(record, dict), "memory record must be an object")
        require(not record.keys() - {"module", "function", "bug_class", "kind", "title", "scans",
                                     "first_scan_id", "last_scan_id", "last_seen_revision"},
                "unknown memory record fields")
        key = key_for(record)
        require(key not in seen, f"duplicate memory key: {key}")
        seen.add(key)
        require(type(record.get("scans")) is int and record["scans"] > 0, "invalid memory scan count")
        require(record.get("kind") in ("FINDING", "LEAD"), "invalid memory kind")
        for field in ("title", "first_scan_id", "last_scan_id", "last_seen_revision"):
            text_field(record.get(field), field)
    return ledger


def memory_before(directory, scope):
    require(scope["memory"], "memory is disabled for this scan")
    ledger_path = Path(text_field(scope.get("ledger_path"), "ledger_path")).resolve()
    require(not ledger_path.is_relative_to(Path(scope["local_checkout"]).resolve()),
            "keep the ledger outside the audited checkout")
    snapshot = read_json(Path(directory) / "memory-before.json")
    return validate_ledger(snapshot, scope)


def snapshot_memory(directory):
    scope, _ = load_scan(directory)
    require(scope["memory"], "memory is disabled for this scan")
    path = Path(text_field(scope.get("ledger_path"), "ledger_path")).resolve()
    require(not path.is_relative_to(Path(scope["local_checkout"]).resolve()),
            "keep the ledger outside the audited checkout")
    snapshot = Path(directory) / "memory-before.json"
    require(not snapshot.exists(), "pre-scan snapshot already exists; do not overwrite it")
    ledger = read_json(path) if path.exists() else dict(
        schema=MEMORY_SCHEMA, repository=scope["repository"], campaign=scope["campaign"], records=[])
    validate_ledger(ledger, scope)
    save_json(snapshot, ledger)
    return ledger


def grouped_items(passes):
    groups = {}
    for run in passes:
        for item in run["items"]:
            groups.setdefault(key_for(item), []).append((run["number"], item))
    return groups


def known(directory):
    scope, passes = load_scan(directory)
    before = memory_before(directory, scope)
    rows = {key_for(record): dict(record, provenance="earlier scan; not revalidated") for record in before["records"]}
    for key, occurrences in grouped_items(passes).items():
        _, item = occurrences[-1]
        rows[key] = dict(item, provenance="earlier pass of this scan; recheck before reporting")
    return list(rows.values())


def update_memory(directory):
    scope, passes = load_scan(directory)
    before = memory_before(directory, scope)
    require(any(run["roles_completed"] for run in passes), "no completed review; ledger unchanged")
    path = Path(scope["ledger_path"])
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.with_name(path.name + ".lock").open("a", encoding="utf-8") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        return merge_memory(path, before, scope, passes)


def merge_memory(path, before, scope, passes):
    if path.exists():
        current = validate_ledger(read_json(path), scope)
        require(current == before or current.get("last_scan_id") == scope["scan_id"],
                "ledger changed since scan start; reconcile concurrent scans before updating")
    else:
        require(not before["records"], "existing ledger disappeared; refusing to replace it")
    records = {key_for(record): dict(record) for record in before["records"]}
    for key, occurrences in grouped_items(passes).items():
        findings = [item for _, item in occurrences if item["kind"] == "FINDING"]
        item = max(findings, key=lambda item: item["confidence"]) if findings else occurrences[-1][1]
        previous = records.get(key)
        records[key] = {field: item[field] for field in ("module", "function", "bug_class", "kind", "title")}
        records[key].update(scans=(previous["scans"] if previous else 0) + 1,
                            first_scan_id=previous["first_scan_id"] if previous else scope["scan_id"],
                            last_scan_id=scope["scan_id"], last_seen_revision=scope["target_revision"])
    ledger = dict(schema=MEMORY_SCHEMA, repository=scope["repository"], campaign=scope["campaign"],
                  last_scan_id=scope["scan_id"], records=list(records.values()))
    save_json(path, ledger)
    return ledger


def cell(value):
    return str(value).replace("|", "\\|").replace("\n", "<br>")


def render_item(item):
    lines = [f"**{item['title']}**", "", item["description"], "",
             "Locations: " + ", ".join(f"`{loc}`" for loc in item["locations"]), ""]
    if item["kind"] == "FINDING":
        lines.extend([f"Severity: {item['severity']} · Confidence: {item['confidence']}", ""])
    for field, label in (("path", "Path"), ("assumptions", "Preconditions"), ("seam", "Combined lenses"),
                         ("proof", "Evidence"), ("fix", "Suggested fix"), ("next_check", "Next check"),
                         ("verification", "Verification")):
        if field in item:
            lines.extend([f"**{label}**", "", item[field], ""])
    for label, detail in item.get("details", {}).items():
        lines.extend([f"**{label}**", "", detail, ""])
    lines.extend(["Candidate IDs: " + ", ".join(item["candidate_ids"]),
                  "Roles: " + ", ".join(item["agent_roles"]), ""])
    return lines


def assemble(directory):
    scope, passes = load_scan(directory)
    before = memory_before(directory, scope) if scope["memory"] else None
    groups = grouped_items(passes)
    findings = [(key, rows) for key, rows in groups.items() if any(i["kind"] == "FINDING" for _, i in rows)]
    leads = [(key, rows) for key, rows in groups.items() if all(i["kind"] == "LEAD" for _, i in rows)]
    def ranking(group):
        items = [i for _, i in group[1] if i["kind"] == "FINDING"]
        return min(SEVERITIES.index(i["severity"]) for i in items), -max(i["confidence"] for i in items), group[0]
    findings.sort(key=ranking)
    lines = [f"# Security audit — {scope['campaign']}", "",
             f"- Model used: {scope['model']}", f"- Reasoning effort: {scope['reasoning_effort']}",
             f"- Audit skill: cl0wdit{scope['skill_version']} ({scope['skill_revision']})", ""]
    for label, field in (("Timestamp", "timestamp"), ("Source repository", "repository"),
                         ("Local checkout", "local_checkout"), ("Target revision", "target_revision"),
                         ("Comparison", "comparison"), ("Mode", "mode"), ("Execution", "execution")):
        lines.append(f"- {label}: {scope[field]}")
    lines.extend(["", "## Coverage", "", "| Pass | Model / effort | Completed roles | Failed roles |", "| --- | --- | --- | --- |"])
    for number in range(1, scope["passes_planned"] + 1):
        run = next((r for r in passes if r["number"] == number), None)
        if run is None:
            lines.append(f"| {number} | — | 0/{len(scope['roles_planned'])} | No saved pass; coverage incomplete |")
        else:
            failed = "; ".join(f"{role}: {reason}" for role, reason in run["roles_failed"].items()) or "—"
            lines.append(f"| {number} | {cell(run['model'])} / {cell(run['reasoning_effort'])} | {len(run['roles_completed'])}/{len(scope['roles_planned'])} | {cell(failed)} |")
    if not any(run["roles_completed"] for run in passes):
        lines.extend(["", "**No review completed. This report does not establish that the code is free of findings.**"])
    lines.extend(["", "Reviewed scope:", ""] + [f"- `{path}`" for path in scope["files"]])
    lines.extend(["", "Limitations and validation:", "", scope.get("limitations") or "No additional limitations recorded.",
                  "", scope.get("validation") or "No execution results recorded.", "", "## Findings overview", "",
                  "| ID | Severity | Confidence | Title |", "| --- | --- | --- | --- |"])
    for number, (_, rows) in enumerate(findings, 1):
        item = max((i for _, i in rows if i["kind"] == "FINDING"), key=lambda i: i["confidence"])
        severity = min((i["severity"] for _, i in rows if i["kind"] == "FINDING"), key=SEVERITIES.index)
        lines.append(f"| F-{number:03} | {severity} | {item['confidence']} | {cell(item['title'])} |")
    if not findings:
        lines.extend(["", "No confirmed findings recorded in the completed review work."])
    for number, (key, rows) in enumerate(findings, 1):
        title = next(i["title"] for _, i in rows if i["kind"] == "FINDING")
        lines.extend(["", f"## F-{number:03} — {title}", "", f"Key: `{key}`", ""])
        if scope["passes_planned"] > 1:
            lines.extend([f"Observed in {len(rows)}/{len(passes)} saved passes; repeated passes are not independent evidence.", ""])
        # Retain every occurrence, including differing proofs, fixes, and uncertain assessments.
        for pass_number, item in rows:
            lines.extend([f"### Pass {pass_number}: {item['kind']}", ""] + render_item(item))
    if leads:
        lines.extend(["", "## Unverified leads", "", "These items are not confirmed findings."])
        for key, rows in leads:
            lines.extend(["", f"### {rows[0][1]['title']}", "", f"Key: `{key}`", ""])
            for number, item in rows:
                lines.extend([f"Pass {number}:", ""] + render_item(item))
    if before is not None:
        historical = [r for r in before["records"] if key_for(r) not in groups]
        lines.extend(["", "## Earlier records not revalidated", "",
                      "Absence from this scan is not evidence that a prior issue is fixed.", ""])
        lines.extend(f"- `{key_for(r)}` — {r['title']} ({r['kind']}; last seen at `{r['last_seen_revision']}`)" for r in historical)
    report = "\n".join(lines).rstrip() + "\n"
    atomic_write(Path(directory) / "full-report.md", report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("validate", "known", "assemble", "snapshot-memory", "memory-update"))
    parser.add_argument("--scan-dir", type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == "assemble":
            print(assemble(args.scan_dir), end="")
        elif args.command == "known":
            print(json.dumps(known(args.scan_dir), ensure_ascii=False, indent=2))
        elif args.command == "memory-update":
            ledger = update_memory(args.scan_dir)
            print(f"Campaign memory: {len(ledger['records'])} records")
        elif args.command == "snapshot-memory":
            ledger = snapshot_memory(args.scan_dir)
            print(f"Saved pre-scan snapshot: {len(ledger['records'])} historical records")
        else:
            scope, passes = load_scan(args.scan_dir)
            if scope["memory"]:
                memory_before(args.scan_dir, scope)
            print(f"Validated {len(passes)}/{scope['passes_planned']} saved passes")
    except (ValueError, KeyError, TypeError, OSError) as error:
        parser.exit(1, f"audit artifacts: {error}\n")


if __name__ == "__main__":
    main()
