#!/bin/sh
# Run the pinned-client terrain oracle (loadTerrain + method9712) on a caller-supplied input.
# Candidate-only: refuses to overwrite output or write inside reference-fixtures/.
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"

usage() {
  echo "usage: run-terrain-oracle.sh --checkout PATH --expected-commit SHA --bcprov JAR --input FILE --output FILE [--work-dir DIR]" >&2
}

CHECKOUT=""; EXPECTED_COMMIT=""; BC=""; INPUT=""; OUTPUT=""; WORK_DIR=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --checkout) CHECKOUT=${2:?}; shift 2 ;;
    --expected-commit) EXPECTED_COMMIT=${2:?}; shift 2 ;;
    --bcprov) BC=${2:?}; shift 2 ;;
    --input) INPUT=${2:?}; shift 2 ;;
    --output) OUTPUT=${2:?}; shift 2 ;;
    --work-dir) WORK_DIR=${2:?}; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage; exit 2 ;;
  esac
done

[ -n "$CHECKOUT" ] && [ -n "$EXPECTED_COMMIT" ] && [ -n "$BC" ] && [ -n "$INPUT" ] && [ -n "$OUTPUT" ] || { usage; exit 2; }
case "$EXPECTED_COMMIT" in *[!0-9a-fA-F]*|'') echo "--expected-commit must be hexadecimal" >&2; exit 2 ;; esac
[ "${#EXPECTED_COMMIT}" -eq 40 ] || { echo "--expected-commit must be 40 hex characters" >&2; exit 2; }
[ -d "$CHECKOUT/.git" ] || { echo "checkout is not a Git worktree: $CHECKOUT" >&2; exit 2; }
[ -f "$BC" ] || { echo "bcprov jar not found: $BC" >&2; exit 2; }
[ -f "$INPUT" ] || { echo "input not found: $INPUT" >&2; exit 2; }
[ ! -e "$OUTPUT" ] || { echo "candidate output already exists: $OUTPUT" >&2; exit 2; }

python3 - "$REPO_ROOT/reference-fixtures" "$OUTPUT" <<'PY'
from pathlib import Path
import sys
fixture_root = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2]).resolve(strict=False)
try:
    output.relative_to(fixture_root)
except ValueError:
    pass
else:
    raise SystemExit("candidate output may not be written inside reference-fixtures")
PY

HEAD=$(git -C "$CHECKOUT" rev-parse HEAD)
[ "$HEAD" = "$EXPECTED_COMMIT" ] || { echo "deob checkout mismatch: expected $EXPECTED_COMMIT, got $HEAD" >&2; exit 2; }

RS="$CHECKOUT/runescape-client/src/main/java"
INJ="$CHECKOUT/injection-annotations/src/main/java"
STUB="$HERE/stubs"

AUTO_WORK=0
if [ -z "$WORK_DIR" ]; then
  WORK_DIR=$(mktemp -d "${TMPDIR:-/tmp}/rustosrs-terrain-oracle.XXXXXX")
  AUTO_WORK=1
else
  mkdir -p "$WORK_DIR"
fi
if [ "$AUTO_WORK" -eq 1 ]; then trap 'rm -rf "$WORK_DIR"' EXIT INT TERM; fi

CLASSES="$WORK_DIR/classes"
mkdir -p "$CLASSES" "$(dirname "$OUTPUT")"

javac -nowarn --patch-module "jdk.jsobject=$STUB" \
  -d "$CLASSES" -cp "$BC" -sourcepath "$RS:$INJ" "$HERE/src/TerrainOracle.java" 2>&1 | grep -v '^Note:' || true

java -cp "$CLASSES" TerrainOracle "$INPUT" > "$OUTPUT"
echo "candidate=$OUTPUT"
