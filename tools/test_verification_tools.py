import copy
import hashlib
import json
import os
import pathlib
import tempfile
import tomllib
import unittest
from types import SimpleNamespace
from unittest.mock import patch

import live_evidence
import verify_portable_core as portable
from continuous_capture import validate_timeline


def fixture():
    proof = dict(passed=True, native_source_worktree_clean=True, features=["acceptance"], source_commit="a" * 40,
                 binary_sha256="b" * 64, native_source_sha256="c" * 64,
                 cargo_lock_sha256="d" * 64, dependencies_lock_sha256="e" * 64)
    binding = dict(schema=1, run_id="1" * 32, build=proof, started_at_unix=1000,
                   finished_at_unix=1100, report_sha256="f" * 64, trace_sha256="2" * 64)
    report = dict(version="0.2.12", run_id=binding["run_id"], passed=True, real_rounds=5,
                  actual_sdk_identities=2, distinct_organizers=2, model_calls=0,
                  both_accounts_created_and_participated=True, activity_roles_and_roster_permissions_verified=True,
                  private_scope_into_public_directory_rejected=True, long_term_preferences_unchanged=True,
                  all_service_event_counts_one=True, controlled_test=True, actual_user_trial=False,
                  cross_machine_trial_completed=False, events=[dict(operation_sha256=hashlib.sha256(str(i).encode()).hexdigest(),
                    service_event_count=1, server_time_ms=1001000, locally_collected_at_unix=1002) for i in range(40)])
    trace = [dict(command="ConfirmParticipant", run_id=binding["run_id"], elapsed_ms=ms, outcome=outcome)
             for ms, outcome in [(10, "success"), (40, "refused"), (100, "unknown")]]
    return report, binding, trace


class VerificationTools(unittest.TestCase):
    def test_relative_result_directory_is_normalized_for_native_export(self):
        from planning_community_acceptance import NextStage
        with tempfile.TemporaryDirectory() as temporary, patch('planning_community_acceptance.Hosts'):
            args=SimpleNamespace(private_directory=pathlib.Path(os.path.relpath(temporary)),executable=pathlib.Path('fixture.exe'),owner_profile=pathlib.Path('owner'),participant_profile=pathlib.Path('member'),version='0.2.12')
            suite=NextStage(args)
            self.assertTrue(suite.out.is_absolute())
            self.assertEqual(suite.out,pathlib.Path(temporary).resolve())
    def test_continuous_video_refuses_missing_time_or_repeated_timestamps(self):
        self.assertEqual(validate_timeline([n/4 for n in range(41)], 4)["captured_seconds"], 10)
        for samples, fps in (([0, 0], 4), ([0, 1, 10], 4), ([0], 4), ([0, .25], 0)):
            with self.assertRaises(ValueError): validate_timeline(samples, fps)
    def test_public_summary_keeps_uncertainty_in_performance_and_discloses_scope(self):
        report, binding, trace = fixture()
        result = live_evidence.summarize(report, binding, trace)
        self.assertFalse(result["judge_verified"])
        self.assertFalse(result["independent_user_trial"])
        self.assertEqual(result["verified_operations"], 40)
        timing = result["command_timings"][0]
        self.assertEqual((timing["samples"], timing["refused"], timing["unknown"], timing["p50_ms"], timing["p95_ms"]), (3, 1, 1, 40, 100))

    def test_public_allowlist_drops_every_raw_private_field(self):
        report, binding, trace = fixture()
        private = "private-account-and-model-content"
        report["account"] = private
        report["events"][0]["raw_event"] = private
        binding["build"]["machine_path"] = private
        trace[0]["result"] = {"token": private, "room": private}
        trace[1]["command"] = private
        result = json.dumps(live_evidence.summarize(report, binding, trace))
        self.assertNotIn(private, result)
        self.assertIn('"Other"', result)

    def test_missing_duplicate_or_multiple_server_effects_refuse_publication(self):
        for change in (lambda r: r["events"].pop(),
                       lambda r: r["events"][0].update(service_event_count=2),
                       lambda r: r["events"][0].update(operation_sha256=r["events"][1]["operation_sha256"]),
                       lambda r: r["events"][0].update(server_time_ms=1000),
                       lambda r: r.update(all_service_event_counts_one=False)):
            report, binding, trace = fixture()
            change(report)
            with self.assertRaises(ValueError): live_evidence.summarize(report, binding, trace)

    def test_stale_report_or_wrong_build_cannot_pass_as_a_fresh_run(self):
        for change in (lambda r, b: r.update(run_id="old"),
                       lambda r, b: b["build"].update(features=["full-host"]),
                       lambda r, b: b["build"].update(native_source_worktree_clean=False),
                       lambda r, b: b["build"].update(source_commit="unknown"),
                       lambda r, b: b.update(report_sha256="unvalidated-input"),
                       lambda r, b: r.update(version="arbitrary-private-content"),
                       lambda r, b: r.update(actual_user_trial=True)):
            report, binding, trace = fixture()
            change(report, binding)
            with self.assertRaises(ValueError): live_evidence.summarize(report, binding, trace)

    def test_invalid_or_missing_timings_cannot_hide_slow_or_failed_commands(self):
        for value in (-1, float("inf"), float("nan"), None, True):
            report, binding, trace = fixture()
            trace[0]["elapsed_ms"] = value
            with self.assertRaises(ValueError): live_evidence.summarize(report, binding, trace)
        report, binding, trace = fixture()
        trace[0]["outcome"] = "ignore"
        with self.assertRaises(ValueError): live_evidence.summarize(report, binding, trace)

    def test_changed_private_inputs_invalidate_the_binding(self):
        report, binding, trace = fixture()
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            for name, content, key in (("acceptance.local.json", report, "report_sha256"), ("trace.private.json", trace, "trace_sha256")):
                (root / name).write_text(json.dumps(content), encoding="utf-8")
                binding[key] = live_evidence.digest(root / name)
            (root / "run-binding.local.json").write_text(json.dumps(binding), encoding="utf-8")
            live_evidence.export(root, root / "public.json")
            (root / "trace.private.json").write_text("[]", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "changed"):
                live_evidence.export(root, root / "changed.json")
            self.assertFalse((root / "changed.json").exists())

    def test_portable_workspace_contains_identical_production_code(self):
        with tempfile.TemporaryDirectory() as temporary:
            target = pathlib.Path(temporary) / "export"
            records, fingerprint = portable.export(target)
            self.assertEqual(len(fingerprint), 64)
            self.assertGreater(len(records), 25)
            for item in records:
                original = portable.ROOT / item["path"]
                copied = target / pathlib.PurePosixPath(item["path"]).relative_to("native/crates")
                self.assertEqual(original.read_bytes(), copied.read_bytes())
            with self.assertRaises(FileExistsError): portable.export(target)

    def test_portable_lock_keeps_desktop_versions_and_checksums(self):
        key = lambda p: (p["name"], p["version"], p.get("source"), p.get("checksum"))
        desktop = tomllib.loads((portable.ROOT / "native/Cargo.lock").read_text("utf-8"))["package"]
        small = tomllib.loads((portable.ROOT / "tools/portable-core.lock").read_text("utf-8"))["package"]
        self.assertLess(len(small), 100)
        self.assertLessEqual({key(p) for p in small}, {key(p) for p in desktop})

    def test_only_complete_passing_test_results_are_accepted(self):
        self.assertEqual(portable.test_counts("test result: ok. 4 passed; 0 failed;\ntest result: ok. 7 passed; 0 failed;"), 11)
        for output in ("", "building", "test result: FAILED. 4 passed; 1 failed;", "test result: ok. 4 passed; 0 failed;\ntest result: FAILED. 0 passed; 1 failed;"):
            with self.assertRaises(ValueError): portable.test_counts(output)


if __name__ == "__main__":
    unittest.main()
