"""Regression checks for the static site and public-repository tooling."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import build_docs_site
from check_repo import Site, check_link, inspect_site
import configure_github


class SiteTests(unittest.TestCase):
    def test_real_site_is_valid(self):
        self.assertEqual(inspect_site(build_docs_site.ROOT), [])

    def test_site_rejects_active_content_and_external_resources(self):
        site = Site()
        site.feed('<script src="https://evil.example/x.js"></script><img src="//evil.example/a"><a onclick="x()" style="color:red">x</a><iframe></iframe>')
        self.assertGreaterEqual(len(site.errors), 5)

    def test_duplicate_ids_rejected(self):
        site = Site()
        site.feed('<h1 id="x">A</h1><p id="x">B</p>')
        self.assertIn("duplicate HTML id: x", site.errors)

    def test_links_cannot_escape_root_or_use_javascript(self):
        root = build_docs_site.ROOT
        source = root / "docs/index.html"
        for link in ["../../outside.txt", "javascript:alert(1)", "#missing", "assets/missing.css"]:
            self.assertIsNotNone(check_link(root, source, link, {"main"}))
        self.assertIsNone(check_link(root, source, "#main", {"main"}))

    def test_staging_is_allowlisted_and_refuses_existing_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "site"
            build_docs_site.build(output)
            files = {p.relative_to(output).as_posix() for p in output.rglob("*") if p.is_file()}
            self.assertEqual(files, set(build_docs_site.FILES))
            with self.assertRaises(ValueError):
                build_docs_site.build(output)
            self.assertTrue((output / "index.html").is_file())

    def test_staging_rejects_linked_assets_before_creating_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "docs").mkdir()
            (root / "outside.html").write_text("private")
            (root / "docs/index.html").symlink_to(root / "outside.html")
            with patch.object(build_docs_site, "ROOT", root), patch.object(build_docs_site, "FILES", ("index.html",)):
                with self.assertRaises(ValueError):
                    build_docs_site.build(root / "output")
            self.assertFalse((root / "output").exists())


class GitHubTests(unittest.TestCase):
    def setUp(self):
        # Mocked API examples must not look like live configuration receipts.
        self.output = contextlib.redirect_stdout(io.StringIO())
        self.output.__enter__()
        self.addCleanup(self.output.__exit__, None, None, None)

    def test_inspection_does_not_write(self):
        calls = []
        def fake(endpoint, method="GET", payload=None):
            calls.append(method)
            return {"full_name": "danieldear/volt-ui-kit", "visibility": "private", "default_branch": "main"}
        with patch("sys.argv", ["configure_github.py"]), patch.object(configure_github, "api", side_effect=fake):
            configure_github.main()
        self.assertTrue(all(method == "GET" for method in calls))

    def test_apply_does_not_change_visibility_or_publish_pages(self):
        calls = []
        policy = json.loads((configure_github.ROOT / ".github/main-protection.json").read_text())
        def fake(endpoint, method="GET", payload=None):
            calls.append((endpoint, method, payload))
            if endpoint.endswith("/protection") and method == "GET":
                return policy
            if endpoint.endswith("/actions/permissions") and method == "GET":
                return {"enabled": True, "allowed_actions": "selected"}
            return {"full_name": "danieldear/volt-ui-kit", "visibility": "public", "default_branch": "main"}
        with patch("sys.argv", ["configure_github.py", "--apply"]), patch.object(configure_github, "api", side_effect=fake):
            configure_github.main()
        self.assertTrue(any(endpoint.endswith("/protection") and method == "PUT" for endpoint, method, _ in calls))
        self.assertFalse(any("visibility" in (payload or {}) or "/pages" in endpoint for endpoint, _, payload in calls))
        self.assertTrue(any(payload and payload.get("allowed_actions") == "selected" for _, _, payload in calls))

    def test_policy_readback_rejects_missing_or_weakened_rules(self):
        policy = json.loads((configure_github.ROOT / ".github/main-protection.json").read_text())
        configure_github.verify_protection(policy, policy)
        for key in ["enforce_admins", "required_conversation_resolution", "allow_force_pushes", "allow_deletions"]:
            wrong = dict(policy)
            wrong[key] = not policy[key]
            with self.assertRaises(RuntimeError):
                configure_github.verify_protection(wrong, policy)
        wrong = dict(policy)
        wrong["required_status_checks"] = {"strict": False, "contexts": []}
        with self.assertRaises(RuntimeError):
            configure_github.verify_protection(wrong, policy)
        wrong = dict(policy)
        wrong["required_pull_request_reviews"] = None
        with self.assertRaises(RuntimeError):
            configure_github.verify_protection(wrong, policy)

    def test_non_main_repository_refused_before_writes(self):
        with patch("sys.argv", ["configure_github.py", "--apply"]), patch.object(configure_github, "api", return_value={"full_name": "owner/repo", "visibility": "private", "default_branch": "other"}) as api:
            with self.assertRaises(RuntimeError):
                configure_github.main()
        self.assertEqual(api.call_count, 1)


if __name__ == "__main__":
    unittest.main()
