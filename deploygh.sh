#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: ./deploygh.sh [--force|-f] <version>"
}

FORCE=0
VERSION_ARG=""
for arg in "$@"; do
  case "$arg" in
    --force|-f)
      FORCE=1
      ;;
    *)
      if [[ -n "$VERSION_ARG" ]]; then
        usage
        exit 1
      fi
      VERSION_ARG="$arg"
      ;;
  esac
done

if [[ -z "$VERSION_ARG" ]]; then
  usage
  exit 1
fi

VERSION="${VERSION_ARG#v}"
TAG="v${VERSION}"

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$ ]]; then
  echo "Invalid version: $VERSION"
  exit 1
fi

command -v git >/dev/null 2>&1 || { echo "git is required"; exit 1; }
command -v gh >/dev/null 2>&1 || { echo "gh is required"; exit 1; }

git rev-parse --is-inside-work-tree >/dev/null 2>&1 || { echo "Run this inside the repo"; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "Run 'gh auth login' first"; exit 1; }

branch="$(git branch --show-current)"
if [[ "$branch" != "main" ]]; then
  echo "Release branch must be main, current branch is: $branch"
  exit 1
fi

dirty_paths="$(
  git status --porcelain --untracked-files=all \
    | sed -E 's/^.. //' \
    | sed -E 's/.* -> //' \
    | grep -v '^Cargo.lock$' \
    || true
)"
if [[ -n "$dirty_paths" ]]; then
  echo "Working tree has non-release changes. Commit or stash before running release."
  printf '%s\n' "$dirty_paths"
  exit 1
fi

git fetch origin --tags
local_tag_exists=0
remote_tag_exists=0
if git rev-parse "$TAG" >/dev/null 2>&1; then
  local_tag_exists=1
fi
if git ls-remote --exit-code --tags origin "refs/tags/$TAG" >/dev/null 2>&1; then
  remote_tag_exists=1
fi
if [[ "$FORCE" -ne 1 ]]; then
  if [[ "$local_tag_exists" -eq 1 ]]; then
    echo "Tag already exists locally: $TAG"
    exit 1
  fi
  if [[ "$remote_tag_exists" -eq 1 ]]; then
    echo "Tag already exists on origin: $TAG"
    exit 1
  fi
fi

if [[ "$FORCE" -eq 1 ]]; then
  echo "Preparing release $TAG (force mode)"
else
  echo "Preparing release $TAG"
fi

perl -i -pe 'BEGIN{$d=0} if(!$d && /^version = ".*"$/){s/^version = ".*"$/version = "'"$VERSION"'"/; $d=1}' Cargo.toml
perl -i -pe 'BEGIN{$d=0} if(!$d && /"version":\s*".*"/){s/"version":\s*".*"/"version": "'"$VERSION"'"/; $d=1}' tauri.conf.json

cargo_version="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
tauri_version="$(sed -n 's/^[[:space:]]*"version": "\(.*\)",$/\1/p' tauri.conf.json | head -n 1)"

if [[ "$cargo_version" != "$VERSION" || "$tauri_version" != "$VERSION" ]]; then
  echo "Version sync failed"
  exit 1
fi

git add Cargo.toml tauri.conf.json Cargo.lock
if ! git diff --cached --quiet; then
  git commit -m "release: $TAG"
fi

git push origin main
sha="$(git rev-parse HEAD)"
if [[ "$FORCE" -eq 1 ]]; then
  git tag -f "$TAG"
  git push --force origin "refs/tags/$TAG"
else
  git tag "$TAG"
  git push origin "$TAG"
fi

echo "Waiting for release workflow run"
run_id=""
for _ in {1..180}; do
  run_id="$(gh run list --workflow release.yml --event push --json databaseId,headSha --limit 100 --jq '.[] | select(.headSha == "'"$sha"'") | .databaseId' | head -n 1 || true)"
  if [[ -n "$run_id" ]]; then
    break
  fi
  sleep 5
done

if [[ -z "$run_id" ]]; then
  echo "Could not find workflow run for commit $sha"
  exit 1
fi

gh run watch "$run_id" --exit-status

required_assets=(
  "flavortime-macos-universal.dmg"
  "flavortime-windows-x64.exe"
  "flavortime-windows-msi-x64.exe"
  "flavortime-linux-x64.AppImage"
  "latest.json"
)
assets="$(gh release view "$TAG" --json assets --jq '.assets[].name')"
for asset in "${required_assets[@]}"; do
  if ! printf '%s\n' "$assets" | grep -qx "$asset"; then
    echo "Missing expected asset in release $TAG: $asset"
    exit 1
  fi
done

if [[ "$(gh release view "$TAG" --json isDraft --jq '.isDraft')" == "true" ]]; then
  gh release edit "$TAG" --draft=false
fi

url="$(gh release view "$TAG" --json url --jq '.url')"
echo "Release published: $url"
