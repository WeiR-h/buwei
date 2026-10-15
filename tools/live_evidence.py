"""Publish an allowlisted summary bound to an actual local SDK test run.

Private reports and traces are inputs only. The public result includes no
account, room, prompt, message, credential or filesystem path. A local run is
always labelled separately from an independent judge or real-user trial.
"""
import argparse
import hashlib
import json
import math
import pathlib
import re
import time
import uuid

HEX = re.compile(r"[0-9a-f]{64}\Z")
COMMIT = re.compile(r"[0-9a-f]{40}\Z")
KINDS = frozenset(("Planning", "Discovery", "Assistance", "Refresh", "SelectActivity", "QuickJoin", "JoinCardAndPrepare", "SuggestDated", "SyncActivity", "ConfirmParticipant", "PreviewAutomation", "ConfirmAutomation", "Accept", "Cancel", "PreviewRosterExport", "ConfirmRosterExport", "CollectEvidence", "Authorize", "ConfirmAuthorization"))


def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def read(path):
    path = pathlib.Path(path)
    if path.stat().st_size > 64 * 1024 * 1024:
        raise ValueError("Evidence input exceeds the supported size")
    return json.loads(path.read_text(encoding="utf-8"))


def begin(executable):
    executable = pathlib.Path(executable)
    proof = read(str(executable) + ".build.json")
    if proof.get("passed") is not True or proof.get("native_source_worktree_clean") is not True or proof.get("binary_sha256") != digest(executable):
        raise ValueError("Executable differs from its measured build")
    if proof.get("features") != ["acceptance"]:
        raise ValueError("This controlled runner requires the separate opt-in acceptance executable")
    if not COMMIT.fullmatch(proof.get("source_commit", "")) or not HEX.fullmatch(proof.get("native_source_sha256", "")):
        raise ValueError("Measured source is missing")
    return {"schema": 1, "run_id": uuid.uuid4().hex, "started_at_unix": int(time.time()), "build": proof}


def finish(binding, executable, directory):
    directory = pathlib.Path(directory)
    if digest(executable) != binding["build"]["binary_sha256"]:
        raise ValueError("Executable changed while testing")
    report = read(directory / "acceptance.local.json")
    if report.get("run_id") != binding["run_id"] or report.get("passed") is not True:
        raise ValueError("A fresh completed report for this run is required")
    binding = dict(binding, finished_at_unix=int(time.time()), report_sha256=digest(directory / "acceptance.local.json"), trace_sha256=digest(directory / "trace.private.json"))
    (directory / "run-binding.local.json").write_text(json.dumps(binding, indent=2) + "\n", encoding="utf-8")
    return binding


def summarize(report, binding, trace):
    if binding.get("schema") != 1 or report.get("run_id") != binding.get("run_id"):
        raise ValueError("Report belongs to a different run")
    proof = binding.get("build", {})
    if proof.get("passed") is not True or proof.get("native_source_worktree_clean") is not True or proof.get("features") != ["acceptance"]:
        raise ValueError("Measured opt-in host build required")
    for key in ("binary_sha256", "native_source_sha256", "cargo_lock_sha256", "dependencies_lock_sha256"):
        if not HEX.fullmatch(proof.get(key, "")):
            raise ValueError("Incomplete build binding")
    if not COMMIT.fullmatch(proof.get("source_commit", "")):
        raise ValueError("Source commit required")
    for key in ("report_sha256", "trace_sha256"):
        if not HEX.fullmatch(binding.get(key, "")):
            raise ValueError("Private input commitment is invalid")
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?", report.get("version", "")):
        raise ValueError("Explicit application version required")
    required = ("passed", "activity_roles_and_roster_permissions_verified", "private_scope_into_public_directory_rejected", "long_term_preferences_unchanged", "all_service_event_counts_one")
    if any(report.get(key) is not True for key in required) or report.get("real_rounds") != 5 or report.get("actual_sdk_identities") != 2 or report.get("model_calls") != 0:
        raise ValueError("The declared two-account five-round checks did not complete")
    if report.get("controlled_test") is not True or report.get("actual_user_trial") is not False or report.get("cross_machine_trial_completed") is not False or report.get("distinct_organizers") != 2:
        raise ValueError("Controlled local test scope must be explicit")
    start, end = binding.get("started_at_unix"), binding.get("finished_at_unix")
    if type(start) is not int or type(end) is not int or end < start:
        raise ValueError("Run clock invalid")
    events = report.get("events", [])
    if len(events) != 40 or len({e.get("operation_sha256") for e in events}) != 40:
        raise ValueError("Expected 40 distinct verified operations")
    public_events = []
    for event in events:
        op, server, collected = event.get("operation_sha256", ""), event.get("server_time_ms"), event.get("locally_collected_at_unix")
        if not HEX.fullmatch(op) or event.get("service_event_count") != 1:
            raise ValueError("Duplicate or unverified server effect")
        if type(server) is not int or type(collected) is not int or not (start - 300 <= server / 1000 <= end + 300) or not (start <= collected <= end):
            raise ValueError("Server and collection clocks must belong to this run")
        public_events.append({"operation_sha256": op, "service_event_count": 1, "server_time_ms": server, "locally_collected_at_unix": collected})
    timings = {}
    for entry in trace:
        if entry.get("run_id") != binding["run_id"]:
            continue
        elapsed = entry.get("elapsed_ms")
        if type(elapsed) not in (int, float) or not math.isfinite(elapsed) or elapsed < 0:
            raise ValueError("Every command needs a real monotonic duration")
        kind = entry.get("command") if entry.get("command") in KINDS else "Other"
        bucket = timings.setdefault(kind, [])
        outcome = entry.get("outcome")
        if outcome not in ("success", "refused", "unknown"):
            raise ValueError("Command outcomes must distinguish refusal and uncertainty")
        bucket.append((elapsed, outcome))
    if not timings:
        raise ValueError("No timings for this run")
    measurements = []
    for kind, values in sorted(timings.items()):
        durations = sorted(v[0] for v in values)
        q = lambda percent: round(durations[math.ceil(len(values) * percent) - 1], 3)
        measurements.append({"kind": kind, "samples": len(values), "refused": sum(v[1] == "refused" for v in values), "unknown": sum(v[1] == "unknown" for v in values), "p50_ms": q(.5), "p95_ms": q(.95), "maximum_ms": round(durations[-1], 3)})
    return {
        "schema": 1, "version": report["version"], "passed": True, "scope": "local_two_account_controlled_run",
        "judge_verified": False, "independent_user_trial": False,
        "activity_scope": "two accounts organize and participate in verified activities; fresh room creation is checked separately",
        "source_commit": proof["source_commit"], "native_source_sha256": proof["native_source_sha256"],
        "binary_sha256": proof["binary_sha256"], "features": ["acceptance"],
        "cargo_lock_sha256": proof["cargo_lock_sha256"], "dependencies_lock_sha256": proof["dependencies_lock_sha256"],
        "started_at_unix": start, "finished_at_unix": end,
        "rounds_per_direction": 5, "distinct_sdk_identities": 2, "distinct_organizers": 2,
        "verified_operations": len(public_events), "maximum_service_events_per_operation": 1,
        "model_calls": 0, "events": public_events,
        "command_timings": measurements,
        "timing_scope": "end-to-end local command completion, including queue and network waits; all observed outcomes included; not frame rate",
        "private_input_commitments": {"report_sha256": binding["report_sha256"], "trace_sha256": binding["trace_sha256"]},
    }


def export(directory, output):
    directory = pathlib.Path(directory)
    binding = read(directory / "run-binding.local.json")
    for name, key in (("acceptance.local.json", "report_sha256"), ("trace.private.json", "trace_sha256")):
        if digest(directory / name) != binding.get(key):
            raise ValueError("Evidence was changed after the run")
    result = summarize(read(directory / "acceptance.local.json"), binding, read(directory / "trace.private.json"))
    output = pathlib.Path(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return {key: result[key] for key in ("passed", "scope", "rounds_per_direction", "verified_operations")}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=pathlib.Path, required=True)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    print(json.dumps(export(args.directory, args.output)))
