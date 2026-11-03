"""Negative acceptance tests for exact-revision candidate qualification."""
from copy import deepcopy
from datetime import datetime, timezone
import unittest

import candidate_evidence as evidence


SHA = "a" * 40
NOW = datetime(2026, 11, 4, tzinfo=timezone.utc)


def snapshot():
    result = {"schema_version": 1, "kind": "candidate", "source_revision": SHA,
              "repository": "Hyper66666/Sengoo", "source_ref": "codex/test", "runs": []}
    for run_id, (workflow, name) in enumerate(evidence.WORKFLOWS.items(), 1):
        result["runs"].append({"id": run_id, "workflow_file": workflow, "name": name,
            "path": f".github/workflows/{workflow}", "head_sha": SHA,
            "head_branch": "codex/test", "head_repository": {"full_name": result["repository"]},
            "event": "pull_request", "status": "completed", "conclusion": "success",
            "run_attempt": 1, "attempt_started_at": "2026-11-03T00:00:00Z",
            "jobs": [{"name": ("python-wheels / " if name.startswith("wheel-") or name == "python-wheel-gate" else "") + name,
                "status": "completed", "conclusion": "success", "steps": [
                    {"name": step, "status": "completed", "conclusion": "success"}
                    for step in ["Verify candidate checkout", *evidence.required_steps(workflow, name)]]}
                for name in evidence.required_jobs(workflow)],
            "artifacts": [{"name": name, "size_in_bytes": 100, "expired": False,
                "expires_at": "2027-01-01T00:00:00Z", "workflow_run": {"head_sha": SHA, "id": run_id}}
                for name in evidence.required_artifacts(workflow, SHA, run_id)]})
        for artifact in result["runs"][-1]["artifacts"]:
            artifact["created_at"] = "2026-11-03T01:00:00Z"
    result["runs"][-1]["jobs"].extend([
        {"name": "published upgrade and rollback", "status": "completed", "conclusion": "skipped"},
        {"name": "publish complete toolchain release", "status": "completed", "conclusion": "skipped"}])
    return result


class CandidateEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.document = snapshot()

    def errors(self):
        return evidence.validate(self.document, NOW)

    def test_complete_candidate_is_separate_from_release_and_pilot(self):
        self.assertEqual([], self.errors())

    def test_missing_workflow_and_wrong_revision_fail(self):
        self.document["runs"].pop()
        self.document["runs"][0]["head_sha"] = "b" * 40
        self.assertGreaterEqual(len(self.errors()), 2)

    def test_matrix_requires_each_target_not_only_count(self):
        realworld = self.document["runs"][4]
        realworld["jobs"][0] = deepcopy(realworld["jobs"][1])
        self.assertTrue(any("count is 0" in error for error in self.errors()))
        self.assertTrue(any("count is 2" in error for error in self.errors()))

    def test_failure_skip_and_missing_checkout_proof_fail(self):
        for mutation in ("skipped", "failure", "cancelled"):
            with self.subTest(mutation=mutation):
                self.document["runs"][0]["jobs"][0]["conclusion"] = mutation
                self.assertTrue(self.errors())
        self.document = snapshot()
        self.document["runs"][0]["jobs"][0]["steps"] = []
        self.assertTrue(any("identity proof" in error for error in self.errors()))

    def test_missing_expired_empty_and_foreign_artifacts_fail(self):
        for mutation in ("missing", "duplicate", "expired", "empty", "foreign", "no_expiry"):
            with self.subTest(mutation=mutation):
                self.document = snapshot()
                artifacts = self.document["runs"][0]["artifacts"]
                if mutation == "missing":
                    artifacts.clear()
                elif mutation == "duplicate":
                    artifacts.append(deepcopy(artifacts[0]))
                elif mutation == "expired":
                    artifacts[0]["expires_at"] = "2026-11-03T00:00:00Z"
                elif mutation == "empty":
                    artifacts[0]["size_in_bytes"] = 0
                elif mutation == "foreign":
                    artifacts[0]["workflow_run"]["head_sha"] = "b" * 40
                else:
                    artifacts[0].pop("expires_at")
                self.assertTrue(self.errors())

    def test_successful_tag_only_publish_is_rejected(self):
        self.document["runs"][-1]["jobs"][-1]["conclusion"] = "success"
        self.assertTrue(any("tag-only" in error for error in self.errors()))

    def test_old_attempt_artifact_cannot_satisfy_current_run(self):
        self.document["runs"][0]["run_attempt"] = 2
        self.document["runs"][0]["attempt_started_at"] = "2026-11-03T02:00:00Z"
        self.assertTrue(any("predates" in error for error in self.errors()))

    def test_required_steps_cannot_skip_or_hide_failure(self):
        for conclusion in ("skipped", "failure"):
            with self.subTest(conclusion=conclusion):
                self.document = snapshot()
                self.document["runs"][0]["jobs"][0]["steps"][-1]["conclusion"] = conclusion
                self.assertTrue(any("required step" in error for error in self.errors()))

    def test_workflow_event_identity_and_fork_are_checked(self):
        for field, value in (("event", "schedule"), ("name", "some-other-workflow"),
                             ("head_repository", {"full_name": "untrusted/Sengoo"}),
                             ("path", ".github/workflows/other.yml")):
            with self.subTest(field=field):
                self.document = snapshot()
                self.document["runs"][0][field] = value
                self.assertTrue(self.errors())

    def test_latest_attempt_is_not_replaced_by_older_green(self):
        class FakeGitHub:
            repository = "Hyper66666/Sengoo"

            def api(self, path):
                return {"head_sha": SHA, "run_attempt": 1, "run_started_at": "2026-11-03T00:00:00Z",
                        "status": None, "conclusion": "failure"}

            def pages(self, path, key):
                if key != "workflow_runs":
                    return []
                base = {"id": 1, "created_at": "2026-11-01T00:00:00Z", "head_sha": SHA,
                    "head_branch": "codex/test", "event": "pull_request", "conclusion": "success"}
                return [base, {**base, "id": 2, "created_at": "2026-11-02T00:00:00Z", "conclusion": "failure"}]

        result = evidence.collect(FakeGitHub(), SHA, "codex/test")
        self.assertFalse(result["ok"])
        self.assertTrue(all(run["id"] == 2 for run in result["runs"]))


if __name__ == "__main__":
    unittest.main()
