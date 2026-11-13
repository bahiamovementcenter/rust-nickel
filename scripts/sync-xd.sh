#!/usr/bin/env bash
# Copies xD's Nickel primitives into nickel-embed/xd/ from a local clone of xD
# at the given commit, and records the commit in nickel-embed/xd/SOURCE.
#
#   scripts/sync-xd.sh ../xD main
set -euo pipefail

xd_repo=${1:?usage: sync-xd.sh <path to an xD clone> <commit or branch>}
ref=${2:?usage: sync-xd.sh <path to an xD clone> <commit or branch>}
dest=$(cd "$(dirname "$0")/../nickel-embed/xd" && pwd)

commit=$(git -C "$xd_repo" rev-parse --verify "$ref^{commit}")
for f in xd.ncl jd.ncl work-packages.ncl tests/primitives.ncl; do
  git -C "$xd_repo" show "$commit:nickel/$f" > "$dest/$f"
done
sed -i.bak "s/^Commit: .*/Commit:     $commit/" "$dest/SOURCE" && rm "$dest/SOURCE.bak"
echo "Copied xD's primitives at $commit. Run cargo test --features xd."
