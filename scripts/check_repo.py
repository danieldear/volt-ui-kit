#!/usr/bin/env python3
"""Offline repository hygiene and documentation-link checks (standard library)."""
import json
from html.parser import HTMLParser
from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
GITHUB = "https://github.com/danieldear/volt-ui-kit/"


class Site(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.ids = set()
        self.links = []
        self.errors = []
        self.h1_count = 0
        self.stylesheets = 0
        self.csp = False

    def handle_starttag(self, tag, attrs):
        a = dict(attrs)
        if "id" in a:
            if a["id"] in self.ids:
                self.errors.append(f"duplicate HTML id: {a['id']}")
            self.ids.add(a["id"])
        self.h1_count += tag == "h1"
        if tag in {"script", "iframe", "form", "object", "embed", "base"}:
            self.errors.append(f"active content is not allowed in static site: {tag}")
        if any(k.startswith("on") or k == "style" for k in a):
            self.errors.append("inline scripts/styles are not allowed")
        if tag == "img" and "alt" not in a:
            self.errors.append("image needs alt text (empty for decorative artwork)")
        if tag == "meta" and a.get("http-equiv", "").lower() == "content-security-policy":
            self.csp = "default-src 'none'" in a.get("content", "")
        for attr in ("href", "src"):
            if attr in a:
                self.links.append(a[attr])
        if tag == "link" and a.get("rel") == "stylesheet":
            self.stylesheets += 1
        if tag in {"img", "link"}:
            resource = a.get("src") or a.get("href", "")
            if urlsplit(resource).scheme or resource.startswith("//"):
                self.errors.append("site assets must be local, not external downloads")


def check_link(root: Path, source: Path, link: str, ids=None):
    parts = urlsplit(link)
    if parts.scheme or parts.netloc:
        if parts.scheme not in {"https", "mailto"}:
            return f"unsupported URL scheme in {source}: {parts.scheme}"
        for prefix in (GITHUB + "blob/main/", GITHUB + "tree/main/"):
            if link.startswith(prefix):
                path = unquote(urlsplit(link[len(prefix):]).path)
                if not (root / path).exists():
                    return f"missing GitHub source target: {path}"
        return None
    path = unquote(parts.path)
    target = (source.parent / path).resolve() if path else source.resolve()
    if not target.is_relative_to(root.resolve()) or not target.exists():
        return f"missing or escaping link in {source.relative_to(root)}: {link}"
    if ids is not None and target == source.resolve() and parts.fragment:
        if unquote(parts.fragment) not in ids:
            return f"missing HTML anchor: {link}"
    elif target.suffix == ".html" and parts.fragment:
        linked = Site()
        linked.feed(target.read_text())
        if unquote(parts.fragment) not in linked.ids:
            return f"missing HTML anchor in {target.relative_to(root)}: {link}"
    return None


def inspect_site(root: Path):
    errors = []
    for source in (root / "docs").rglob("*.html"):
        site = Site()
        site.feed(source.read_text())
        errors.extend(f"{source.relative_to(root)}: {e}" for e in site.errors)
        if site.h1_count != 1 or not 1 <= site.stylesheets <= 2 or not site.csp:
            errors.append(f"{source.relative_to(root)}: site requires one h1, local stylesheets and a restrictive CSP")
        for link in site.links:
            error = check_link(root, source, link, site.ids)
            if error:
                errors.append(error)
    return errors


def candidates():
    result = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=ROOT, check=True, capture_output=True,
    )
    return sorted(set(Path(p.decode()) for p in result.stdout.split(b"\0") if p))


def main():
    errors = inspect_site(ROOT)
    files = candidates()
    forbidden = re.compile(r"(?:^|/)(?:\.env(?:\..*)?|id_rsa|id_ed25519|credentials\.json)$|\.(?:p12|pfx|pem|key|mobileprovision|provisionprofile)$", re.I)
    for path in files:
        if forbidden.search(path.as_posix()) and path.name != ".env.example":
            errors.append(f"credential-shaped file must not be tracked: {path}")
        absolute = ROOT / path
        if absolute.is_symlink():
            errors.append(f"review repository symlink before publishing: {path}")
        if path.suffix == ".md" and absolute.is_file():
            text = re.sub(r"```.*?```", "", absolute.read_text(), flags=re.S)
            for link in re.findall(r"\[[^\]]*\]\(([^\s)]+)(?:\s+\"[^\"]*\")?\)", text):
                error = check_link(ROOT, absolute, link)
                if error:
                    errors.append(error)
    # A public PR must never execute code with a writable target-branch token.
    for path in (ROOT / ".github/workflows").glob("*.yml"):
        workflow = path.read_text()
        if "pull_request_target:" in workflow or "secrets: inherit" in workflow:
            errors.append(f"privileged PR execution requires explicit security review: {path.name}")
        for action in re.findall(r"uses:\s*([^\s#]+)", workflow):
            if not re.fullmatch(r"[\w.-]+/[\w./-]+@[0-9a-f]{40}", action):
                errors.append(f"action must be pinned to a full SHA: {action}")
    policy = json.loads((ROOT / ".github/main-protection.json").read_text())
    if not policy["enforce_admins"] or policy["allow_force_pushes"] or policy["allow_deletions"]:
        errors.append("main policy must protect admins and disallow force pushes/deletion")
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print(f"Repository hygiene passed ({len(files)} files; local HTML/Markdown targets and pinned actions checked).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
