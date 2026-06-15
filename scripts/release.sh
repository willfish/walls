#!/usr/bin/env bash
set -euo pipefail

usage() {
	echo "Usage: scripts/release.sh <version>"
	echo
	echo "Example: scripts/release.sh 0.14.0"
}

if [[ ${1:-} == "-h" || ${1:-} == "--help" ]]; then
	usage
	exit 0
fi

version="${1:-}"
if [[ -z $version ]]; then
	usage >&2
	exit 2
fi

version="${version#v}"
if [[ ! $version =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]; then
	echo "error: version must be semver-like, for example 0.14.0" >&2
	exit 2
fi

tag="v${version}"
date="${RELEASE_DATE:-$(date +%F)}"
branch="$(git rev-parse --abbrev-ref HEAD)"

if [[ $branch != "main" ]]; then
	echo "error: releases must be cut from main, currently on ${branch}" >&2
	exit 1
fi

if ! git diff --quiet || ! git diff --cached --quiet; then
	echo "error: working tree must be clean before cutting a release" >&2
	exit 1
fi

repo="$(gh repo view --json nameWithOwner --jq .nameWithOwner)"
repo_url="https://github.com/${repo}"
latest_tag="$(git describe --tags --abbrev=0 2>/dev/null || true)"

if [[ $latest_tag == "$tag" ]]; then
	echo "error: ${tag} already exists locally" >&2
	exit 1
fi

immutable_state="$(gh api "repos/${repo}/immutable-releases" --jq '.enabled' 2>/dev/null || echo false)"
if [[ $immutable_state == "true" ]]; then
	gh api --method DELETE "repos/${repo}/immutable-releases" --silent
fi

range="HEAD"
if [[ -n $latest_tag ]]; then
	range="${latest_tag}..HEAD"
fi

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

nix-shell -p cocogitto --run "cog changelog ${range}" >"${tmpdir}/cog.md"
sed '1d; s/^#### /### /' "${tmpdir}/cog.md" >"${tmpdir}/notes.md"

if [[ ! -s "${tmpdir}/notes.md" ]]; then
	echo "error: no changelog entries generated for ${range}" >&2
	exit 1
fi

VERSION="$version" perl -0pi -e 's/(\[workspace\.package\]\nversion = ")[^"]+(")/$1$ENV{VERSION}$2/' Cargo.toml
cargo update -w

awk -v version="$version" -v date="$date" -v notes="${tmpdir}/notes.md" '
  BEGIN {
    while ((getline line < notes) > 0) {
      body = body line "\n"
    }
    close(notes)
  }
  {
    print
    if ($0 == "## [Unreleased]" && inserted == 0) {
      print ""
      print "## [" version "] - " date
      printf "%s", body
      inserted = 1
    }
  }
' CHANGELOG.md >"${tmpdir}/CHANGELOG.md"
mv "${tmpdir}/CHANGELOG.md" CHANGELOG.md

unreleased_link="[unreleased]: ${repo_url}/compare/${tag}...HEAD"
if [[ -n $latest_tag ]]; then
	release_link="[${version}]: ${repo_url}/compare/${latest_tag}...${tag}"
else
	release_link="[${version}]: ${repo_url}/releases/tag/${tag}"
fi

perl -0pi -e "s#\\[unreleased\\]: .*#${unreleased_link}\n${release_link}#" CHANGELOG.md

cargo fmt --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings

git add CHANGELOG.md Cargo.toml Cargo.lock cog.toml scripts/release.sh
git commit -m "chore(release): release ${tag}" -m "Issue: No ticket/issue"
nix-shell -p cocogitto --run "cog check HEAD~1..HEAD"
git push origin main
git -c tag.gpgSign=false tag -f "$tag" HEAD
git push --force origin "refs/tags/${tag}"

awk -v version="$version" '
  $0 == "## [" version "] - " d { print_section = 1; next }
  /^## \[/ && print_section == 1 { exit }
  print_section == 1 { print }
' d="$date" CHANGELOG.md >"${tmpdir}/release-notes.md"

if gh release view "$tag" >/dev/null 2>&1; then
	gh release edit "$tag" --title "$version" --notes-file "${tmpdir}/release-notes.md" --latest
else
	gh release create "$tag" --target main --title "$version" --notes-file "${tmpdir}/release-notes.md" --latest
fi
