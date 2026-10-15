#!/usr/bin/env python3
"""Inspect or apply reviewed GitHub settings. Never changes repository visibility."""
import argparse
import json
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def api(endpoint, method="GET", payload=None):
    args = ["gh", "api", "--method", method, endpoint]
    if payload is not None:
        args += ["--input", "-"]
    result = subprocess.run(args, input=json.dumps(payload) if payload is not None else None,
                            text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(result.stderr.strip())
    return json.loads(result.stdout) if result.stdout.strip() else None


def verify_protection(actual, policy):
    def enabled(value):
        return value.get("enabled", False) if isinstance(value, dict) else bool(value)
    checks = actual.get("required_status_checks") or {}
    if set(checks.get("contexts", [])) != set(policy["required_status_checks"]["contexts"]) or not checks.get("strict"):
        raise RuntimeError("required-check readback differs from policy")
    reviews = actual.get("required_pull_request_reviews") or {}
    expected = policy["required_pull_request_reviews"]
    if any(reviews.get(key) != value for key, value in expected.items()):
        raise RuntimeError("PR-review readback differs from policy")
    for key, expected in (("enforce_admins", True), ("required_conversation_resolution", True),
                          ("allow_force_pushes", False), ("allow_deletions", False)):
        if key not in actual or enabled(actual[key]) != expected:
            raise RuntimeError(f"protection readback differs from policy: {key}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default="danieldear/volt-ui-kit")
    parser.add_argument("--apply", action="store_true", help="apply repository and main-branch policy")
    parser.add_argument("--reviews", type=int, choices=range(0, 7), default=0)
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", args.repo):
        parser.error("--repo must be OWNER/REPO")
    endpoint = f"repos/{args.repo}"
    repo = api(endpoint)
    print(f"{repo['full_name']}: {repo['visibility']}; default branch={repo['default_branch']}")
    if repo["default_branch"] != "main":
        raise RuntimeError("expected main as default branch; refusing to change another branch")
    policy = json.loads((ROOT / ".github/main-protection.json").read_text())
    policy["required_pull_request_reviews"]["required_approving_review_count"] = args.reviews
    policy["required_pull_request_reviews"]["require_code_owner_reviews"] = args.reviews > 0
    print(f"Requested policy: PRs, {args.reviews} approvals, strict checks, conversation resolution, "
          "admin enforcement, no force pushes/deletion.")
    if not args.apply:
        print("Read-only inspection. Use --apply only after required checks have run successfully.")
        try:
            print(json.dumps(api(endpoint + "/branches/main/protection"), indent=2))
        except RuntimeError as error:
            print(f"Protection unavailable: {error}")
        return
    # Deliberately no visibility, Pages creation, paid security features, or history rewriting.
    api(endpoint, "PATCH", {"delete_branch_on_merge": True})
    api(endpoint + "/actions/permissions/workflow", "PUT", {
        "default_workflow_permissions": "read", "can_approve_pull_request_reviews": False,
    })
    permissions = api(endpoint + "/actions/permissions")
    api(endpoint + "/actions/permissions", "PUT", {
        "enabled": permissions["enabled"],
        "allowed_actions": permissions["allowed_actions"],
        "sha_pinning_required": True,
    })
    api(endpoint + "/vulnerability-alerts", "PUT")
    # Fail rather than claiming protection on unsupported private/free plans.
    api(endpoint + "/branches/main/protection", "PUT", policy)
    actual = api(endpoint + "/branches/main/protection")
    verify_protection(actual, policy)
    if repo["visibility"] == "public":
        api(endpoint + "/private-vulnerability-reporting", "PUT")
        api(endpoint, "PATCH", {"security_and_analysis": {
            "secret_scanning": {"status": "enabled"},
            "secret_scanning_push_protection": {"status": "enabled"},
        }})
    final = api(endpoint)
    if final["visibility"] != repo["visibility"]:
        raise RuntimeError("visibility changed concurrently; review repository settings")
    print("Policy applied; required checks verified. Review GitHub settings before publication.")


if __name__ == "__main__":
    try:
        main()
    except RuntimeError as error:
        raise SystemExit(str(error))
