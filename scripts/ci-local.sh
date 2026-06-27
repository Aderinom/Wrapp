#!/usr/bin/env sh
# Run the same checks as .github/workflows/ci.yml locally, using the *real*
# aderinom/rust-all-action bundle (no source changes to the action itself).
#
# The action is vendored into a git-ignored, repo-relative folder (.tools/),
# so this never depends on a path outside the repository.
#
# Usage:
#   scripts/ci-local.sh                 # fmt, clippy, shear, deny, test, doc
#   RUN=clippy scripts/ci-local.sh      # run a single workflow
#   RUN=fmt,clippy scripts/ci-local.sh  # run a subset (no spaces!)
#   ACTION_REF=main scripts/ci-local.sh # pin a different action ref
#   VERBOSE=1 scripts/ci-local.sh       # also show ::debug:: lines
#   NO_COLOR=1 scripts/ci-local.sh      # disable colored output
set -eu

# --- pretty output --------------------------------------------------------
# The action speaks GitHub's workflow-command dialect (::group::, ::warning::,
# ...). Locally we translate those into readable, colored terminal output.
if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
  COLOR=1
  C_RESET=$(printf "\033[0m"); C_BOLD=$(printf "\033[1m")
  C_CYAN=$(printf "\033[36m"); C_RED=$(printf "\033[31m")
  C_GRN=$(printf "\033[32m");  C_YEL=$(printf "\033[33m")
else
  COLOR=0
  C_RESET=; C_BOLD=; C_CYAN=; C_RED=; C_GRN=; C_YEL=
fi
VERBOSE="${VERBOSE:-0}"

say() { printf "%s>>%s %s\n" "$C_BOLD$C_CYAN" "$C_RESET" "$*"; }
die() { printf "%s!!%s %s\n" "$C_BOLD$C_RED" "$C_RESET" "$*" >&2; exit 1; }

# --- config ---------------------------------------------------------------
ACTION_REPO="https://github.com/aderinom/rust-all-action.git"
ACTION_REF="${ACTION_REF:-v1}"     # matches `uses: aderinom/rust-all-action@v1`
RUN="${RUN:-all-default,deny}"     # all-default = fmt,clippy,shear,test,doc
TOOLCHAIN="${TOOLCHAIN:-stable}"   # rust-toolchain.toml channel still wins

# --- paths ----------------------------------------------------------------
REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
VENDOR_DIR="$REPO_ROOT/.tools/rust-all-action"
ENTRY="$VENDOR_DIR/dist/index.js"

# --- fetch / update the vendored action bundle ----------------------------
if [ ! -d "$VENDOR_DIR/.git" ]; then
  say "Cloning rust-all-action@$ACTION_REF into .tools/ ..."
  git clone --quiet --depth 1 --branch "$ACTION_REF" "$ACTION_REPO" "$VENDOR_DIR"
else
  say "Updating rust-all-action to $ACTION_REF ..."
  git -C "$VENDOR_DIR" fetch --quiet --depth 1 origin "$ACTION_REF"
  git -C "$VENDOR_DIR" checkout --quiet --force FETCH_HEAD
fi

[ -f "$ENTRY" ] || die "Bundled entry point not found: $ENTRY"

# --- run ------------------------------------------------------------------
# GitHub Actions passes inputs as INPUT_<NAME> env vars; the action reads them
# via @actions/core.getInput. cacheKey=no-cache disables the GitHub-only cache
# service calls, leaving tool install + workflow execution intact.
say "Running workflows: $RUN"
cd "$REPO_ROOT"

# awk filter: translate GitHub workflow commands into readable terminal output
# and indent everything emitted inside a ::group::.
FILTER='
function decode(s) {
  gsub(/%0D/, "\r", s)
  gsub(/%0A/, "\n", s)
  gsub(/%3A/, ":", s)
  gsub(/%2C/, ",", s)
  gsub(/%25/, "%", s)
  return s
}
BEGIN {
  esc = sprintf("%c", 27)
  if (color == "1") {
    reset = esc "[0m"; bold = esc "[1m"; dim = esc "[2m"
    red = esc "[31m"; grn = esc "[32m"; yel = esc "[33m"; cyn = esc "[36m"
  }
}
{
  if (substr($0, 1, 2) == "::") {
    rest = substr($0, 3)
    idx = index(rest, "::")
    if (idx > 0) {
      header = substr(rest, 1, idx - 1)
      msg = decode(substr(rest, idx + 2))
      split(header, parts, " ")
      cmd = parts[1]
      if (cmd == "group")    { printf "\n%s%s▶ %s%s\n", bold, cyn, msg, reset; ingroup = 1; next }
      if (cmd == "endgroup") { ingroup = 0; next }
      if (cmd == "debug")    { if (verbose == "1") printf "%s  · %s%s\n", dim, msg, reset; next }
      if (cmd == "warning")  { if (index(msg, "SCCACHE_PATH is not set") == 0) printf "%s⚠ %s%s\n", yel, msg, reset; next }
      if (cmd == "error")    { printf "%s✖ %s%s\n", red, msg, reset; next }
      if (cmd == "notice")   { printf "%s● %s%s\n", grn, msg, reset; next }
      next
    }
  }
  if (ingroup == 1) print "  " $0; else print $0
}
'

# Run the action, but keep ITS exit code (not awk's) so failures still fail.
# POSIX sh has no pipefail, so stash the status in a temp file.
status_file=$(mktemp)
trap 'rm -f "$status_file"' EXIT INT TERM

set +e
{
  INPUT_PROJECT="." \
  INPUT_RUN="$RUN" \
  INPUT_TOOLCHAIN="$TOOLCHAIN" \
  INPUT_CACHEKEY="no-cache" \
  node "$ENTRY" 2>&1
  echo $? > "$status_file"
} | awk -v color="$COLOR" -v verbose="$VERBOSE" "$FILTER"
set -e

code=$(cat "$status_file" 2>/dev/null || echo 1)
case "$code" in (""|*[!0-9]*) code=1 ;; esac

echo
if [ "$code" -eq 0 ]; then
  printf "%s✔ All workflows passed%s (%s)\n" "$C_BOLD$C_GRN" "$C_RESET" "$RUN"
elif [ "$code" -eq 130 ]; then
  printf "%s■ Interrupted%s\n" "$C_BOLD$C_YEL" "$C_RESET"
else
  printf "%s✖ Workflow(s) failed%s (exit %s)\n" "$C_BOLD$C_RED" "$C_RESET" "$code"
fi
exit "$code"
