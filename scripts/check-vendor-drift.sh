#!/usr/bin/env bash
# Diff vendored src-tauri/src/core/*.rs against their upstream source in
# synle/display-dj-cli. See VENDORING.md for the file map and recorded SHAs.
#
# Usage:
#   ./scripts/check-vendor-drift.sh           # report; exit 1 on drift
#   ./scripts/check-vendor-drift.sh --update  # rewrite SHAs in VENDORING.md to upstream main HEAD
#
# Override upstream checkout location:
#   DISPLAY_DJ_CLI_PATH=/path/to/display-dj-cli ./scripts/check-vendor-drift.sh
set -euo pipefail

CLI_PATH="${DISPLAY_DJ_CLI_PATH:-/Users/syle/git/display-dj-cli}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VENDORING_MD="$REPO_ROOT/VENDORING.md"

UPDATE=0
[[ "${1:-}" == "--update" ]] && UPDATE=1

if [ ! -d "$CLI_PATH/.git" ]; then
  echo "error: display-dj-cli not found at $CLI_PATH" >&2
  echo "       override with DISPLAY_DJ_CLI_PATH=/path/to/display-dj-cli" >&2
  exit 2
fi

if ! git -C "$CLI_PATH" remote get-url origin 2>/dev/null | grep -q "synle/display-dj-cli"; then
  echo "error: $CLI_PATH does not point at synle/display-dj-cli" >&2
  echo "       \`git -C $CLI_PATH remote -v\` should mention synle/display-dj-cli" >&2
  exit 2
fi

if [ ! -f "$VENDORING_MD" ]; then
  echo "error: $VENDORING_MD not found" >&2
  exit 2
fi

HEAD_SHA_FULL="$(git -C "$CLI_PATH" rev-parse main)"
HEAD_SHA="$(git -C "$CLI_PATH" rev-parse --short=7 main)"

# Compute distance between two SHAs on cli/main (commits between, may be 0).
# Prints either "no drift" or "drift +N commits".
drift_label() {
  local from="$1" to="$2"
  if [ "$(git -C "$CLI_PATH" rev-parse "$from^{commit}" 2>/dev/null)" = \
       "$(git -C "$CLI_PATH" rev-parse "$to^{commit}" 2>/dev/null)" ]; then
    echo "no drift"
    return
  fi
  local count
  count="$(git -C "$CLI_PATH" rev-list --count "$from..$to" 2>/dev/null || echo "?")"
  echo "drift +$count commits"
}

# Parse vendoring table rows. Pipe-style markdown table rows where:
#   col1 = `src-tauri/src/core/<file>`
#   col2 = `<upstream/path>`
#   col3 = `<sha>`
# Skip header/separator/blank rows.
rows="$(awk -F'|' '
  /^\| `src-tauri\/src\/core\// {
    # strip leading/trailing pipes and whitespace + backticks from cells 2,3,4
    v=$2; u=$3; s=$4
    gsub(/^ +| +$/, "", v); gsub(/^`|`$/, "", v)
    gsub(/^ +| +$/, "", u); gsub(/^`|`$/, "", u)
    gsub(/^ +| +$/, "", s); gsub(/^`|`$/, "", s)
    print v "\t" u "\t" s
  }
' "$VENDORING_MD")"

if [ -z "$rows" ]; then
  echo "error: no vendoring rows parsed from $VENDORING_MD" >&2
  exit 2
fi

echo "Upstream: synle/display-dj-cli @ $CLI_PATH"
echo "main HEAD: $HEAD_SHA ($HEAD_SHA_FULL)"
echo ""

drift_count=0
sha_changes=""
printf "%-38s %-12s %-26s %s\n" "FILE" "RECORDED" "UPSTREAM-AT-RECORDED" "UPSTREAM-HEAD"
printf "%-38s %-12s %-26s %s\n" "----" "--------" "--------------------" "-------------"

while IFS=$'\t' read -r vendored upstream recorded; do
  [ -z "$vendored" ] && continue
  vfile="$REPO_ROOT/$vendored"
  ufile="$upstream"

  if [ ! -f "$vfile" ]; then
    printf "%-38s %-12s %s\n" "$vendored" "$recorded" "missing: $vfile"
    drift_count=$((drift_count + 1))
    continue
  fi

  # Check recorded SHA exists in upstream
  if ! git -C "$CLI_PATH" cat-file -e "$recorded^{commit}" 2>/dev/null; then
    printf "%-38s %-12s %s\n" "$vendored" "$recorded" "unknown sha in upstream"
    drift_count=$((drift_count + 1))
    continue
  fi

  # Does the file exist at the recorded sha?
  if ! git -C "$CLI_PATH" cat-file -e "$recorded:$ufile" 2>/dev/null; then
    at_recorded="missing upstream file @$recorded"
  else
    at_recorded="ok"
  fi

  # Drift from recorded SHA → main HEAD on this file only.
  if [ "$recorded" = "$HEAD_SHA" ] || \
     [ "$(git -C "$CLI_PATH" rev-parse "$recorded^{commit}")" = "$HEAD_SHA_FULL" ]; then
    head_state="no drift"
  else
    file_commits="$(git -C "$CLI_PATH" rev-list --count "$recorded..main" -- "$ufile" 2>/dev/null || echo "?")"
    if [ "$file_commits" = "0" ]; then
      head_state="no file drift (head=$HEAD_SHA)"
    else
      head_state="drift +$file_commits commits on $ufile (head=$HEAD_SHA)"
      drift_count=$((drift_count + 1))
    fi
  fi

  printf "%-38s %-12s %-26s %s\n" "$vendored" "$recorded" "$at_recorded" "$head_state"
  sha_changes+="$vendored"$'\t'"$recorded"$'\t'"$HEAD_SHA"$'\n'
done <<< "$rows"

echo ""
if [ $UPDATE -eq 1 ]; then
  while IFS=$'\t' read -r v old new; do
    [ -z "$v" ] && continue
    [ "$old" = "$new" ] && continue
    # Replace the first occurrence of `<old>` on the row mentioning <v>.
    # Use a python-free in-place edit via awk -> mv.
    tmp="$(mktemp)"
    awk -v v="$v" -v old="$old" -v new="$new" '
      $0 ~ ("`" v "`") { sub("`" old "`", "`" new "`") }
      { print }
    ' "$VENDORING_MD" > "$tmp"
    mv "$tmp" "$VENDORING_MD"
  done <<< "$sha_changes"
  echo "updated SHAs in $VENDORING_MD to upstream main HEAD ($HEAD_SHA)"
  exit 0
fi

if [ $drift_count -gt 0 ]; then
  echo "DRIFT: $drift_count vendored file(s) have moved upstream since their recorded SHA."
  echo "       Either re-vendor (copy upstream → local, re-apply adaptations) and run with --update,"
  echo "       or update VENDORING.md manually to reflect the new baseline."
  exit 1
fi

echo "OK: all vendored files match their recorded upstream SHAs."
