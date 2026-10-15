"""Reproduce the production receipt and activity rules without a desktop host.

The exported workspace contains byte-identical production sources, not mocks.
Its small, checked-in lock uses the same package versions as the desktop lock.
No credentials, Matrix sessions or Windows-only dependencies are needed.
"""
import argparse
import hashlib
import json
import pathlib
import platform
import re
import shutil
import subprocess
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
CRATES = ("action-receipts", "buwei-host-core")


def source_files(root=ROOT):
    files = []
    for name in CRATES:
        base = root / "native/crates" / name
        for path in sorted(base.rglob("*")):
            relative = path.relative_to(base)
            if any(part in {"target", ".git", "__pycache__"} for part in relative.parts):
                continue
            if path.is_file() and (path.suffix == ".rs" or relative.as_posix() == "Cargo.toml"):
                if path.is_symlink() or not path.resolve().is_relative_to(base.resolve()):
                    raise ValueError("Portable source must remain inside the production crate")
                files.append(path)
    return files


def export(destination, root=ROOT):
    destination = pathlib.Path(destination)
    destination.mkdir(parents=True, exist_ok=False)
    records = []
    for source in source_files(root):
        relative = source.relative_to(root / "native/crates")
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        content = source.read_bytes()
        target.write_bytes(content)
        records.append({"path": "native/crates/" + relative.as_posix(), "sha256": hashlib.sha256(content).hexdigest()})
    (destination / "Cargo.toml").write_text('[workspace]\nmembers = ["action-receipts", "buwei-host-core"]\nresolver = "3"\n', encoding="utf-8")
    shutil.copyfile(root / "tools/portable-core.lock", destination / "Cargo.lock")
    fingerprint = hashlib.sha256(json.dumps(records, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return records, fingerprint


def test_counts(output):
    results = re.findall(r"test result: (ok|FAILED)\. (\d+) passed; (\d+) failed;", output)
    if not results or any(status != "ok" or int(failed) for status, _, failed in results):
        raise ValueError("No complete passing Rust test results")
    return sum(int(passed) for _, passed, _ in results)


def verify(destination, output, offline=False, benchmark_output=None):
    records, fingerprint = export(destination)
    manifest = str(pathlib.Path(destination).resolve() / "Cargo.toml")
    if not offline:
        subprocess.run(["cargo", "fetch", "--manifest-path", manifest, "--locked"], check=True)
    started = time.perf_counter()
    result = subprocess.run(["cargo", "test", "--manifest-path", manifest, "--workspace", "--all-targets", "--locked", "--offline"], text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, encoding="utf-8", errors="replace")
    print(result.stdout, flush=True)
    result.check_returncode()
    count = test_counts(result.stdout)
    benchmark = None
    if benchmark_output is not None:
        measured = subprocess.run(["cargo", "run", "--manifest-path", manifest, "-p", "action-receipts", "--example", "benchmark_recovery", "--locked", "--offline"], text=True, capture_output=True, encoding="utf-8", errors="replace")
        if measured.returncode:
            print(measured.stderr, flush=True)
            measured.check_returncode()
        benchmark = json.loads(measured.stdout)
        if benchmark.get("passed") is not True or benchmark.get("results_equal") is not True or benchmark.get("network_calls") != 0:
            raise ValueError("Journal benchmark did not verify equal results")
    # Detect edits during the check rather than attesting a different source.
    for item in records:
        if hashlib.sha256((ROOT / item["path"]).read_bytes()).hexdigest() != item["sha256"]:
            raise ValueError("Production source changed during validation")
    report = {
        "schema": 1, "passed": True, "kind": "portable-production-core",
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "source_worktree_clean": not subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=all", "--", "native/crates"], cwd=ROOT, text=True).strip(),
        "source_sha256": fingerprint, "source_files": records,
        "lock_sha256": hashlib.sha256((ROOT / "tools/portable-core.lock").read_bytes()).hexdigest(),
        "os": platform.system(), "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "tests_passed": count, "elapsed_seconds": round(time.perf_counter() - started, 3),
        "network_during_tests": False, "model_calls": 0,
        "scope": "Production rule and receipt tests; desktop rendering and live service behavior require separate verification.",
    }
    output = pathlib.Path(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if benchmark is not None:
        benchmark.update(source_commit=report["source_commit"], source_sha256=fingerprint)
        benchmark_output = pathlib.Path(benchmark_output)
        benchmark_output.parent.mkdir(parents=True, exist_ok=True)
        benchmark_output.write_text(json.dumps(benchmark, indent=2) + "\n", encoding="utf-8")
    return {key: report[key] for key in ("passed", "kind", "os", "tests_passed", "elapsed_seconds", "source_sha256")}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--destination", type=pathlib.Path, required=True, help="A new empty workspace")
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--benchmark-output", type=pathlib.Path, help="Also compare journal queries before/after the index upgrade")
    args = parser.parse_args()
    print(json.dumps(verify(args.destination, args.output, args.offline, args.benchmark_output)))
