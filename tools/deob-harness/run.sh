#!/bin/sh
# Build + run the deob headless harness into a candidate artifact.
# Requires: git, javac, java, python3, and a caller-supplied bcprov 1.52 jar.
set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"

usage() {
  cat <<'EOF'
usage: run.sh --checkout PATH --expected-commit SHA --bcprov JAR --output FILE [--work-dir DIR]

Runs the headless deob harness from an explicitly supplied source checkout.
The checkout must be exactly at EXPECTED-COMMIT. Output is candidate-only:
paths inside this repository's reference-fixtures/ directory are rejected.
EOF
}

CHECKOUT=""
EXPECTED_COMMIT=""
BC=""
OUTPUT=""
WORK_DIR=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --checkout)
      CHECKOUT=${2:?missing value for --checkout}
      shift 2
      ;;
    --expected-commit)
      EXPECTED_COMMIT=${2:?missing value for --expected-commit}
      shift 2
      ;;
    --bcprov)
      BC=${2:?missing value for --bcprov}
      shift 2
      ;;
    --output)
      OUTPUT=${2:?missing value for --output}
      shift 2
      ;;
    --work-dir)
      WORK_DIR=${2:?missing value for --work-dir}
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[ -n "$CHECKOUT" ] || { echo "--checkout is required" >&2; exit 2; }
[ -n "$EXPECTED_COMMIT" ] || { echo "--expected-commit is required" >&2; exit 2; }
[ -n "$BC" ] || { echo "--bcprov is required" >&2; exit 2; }
[ -n "$OUTPUT" ] || { echo "--output is required" >&2; exit 2; }

case "$EXPECTED_COMMIT" in
  *[!0-9a-fA-F]*|'') echo "--expected-commit must be hexadecimal" >&2; exit 2 ;;
esac
[ "${#EXPECTED_COMMIT}" -eq 40 ] || { echo "--expected-commit must be 40 hex characters" >&2; exit 2; }

[ -d "$CHECKOUT/.git" ] || { echo "checkout is not a Git worktree: $CHECKOUT" >&2; exit 2; }
[ -f "$BC" ] || { echo "bcprov jar not found: $BC" >&2; exit 2; }
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
[ "$HEAD" = "$EXPECTED_COMMIT" ] || {
  echo "deob checkout mismatch: expected $EXPECTED_COMMIT, got $HEAD" >&2
  exit 2
}

RS="$CHECKOUT/runescape-client/src/main/java"
INJ="$CHECKOUT/injection-annotations/src/main/java"
STUB="$HERE/stubs"
[ -d "$RS" ] || { echo "runescape-client source root missing: $RS" >&2; exit 2; }
[ -d "$INJ" ] || { echo "injection-annotations source root missing: $INJ" >&2; exit 2; }

AUTO_WORK=0
if [ -z "$WORK_DIR" ]; then
  WORK_DIR=$(mktemp -d "${TMPDIR:-/tmp}/rustosrs-deob.XXXXXX")
  AUTO_WORK=1
else
  mkdir -p "$WORK_DIR"
fi
if [ "$AUTO_WORK" -eq 1 ]; then
  trap 'rm -rf "$WORK_DIR"' EXIT INT TERM
fi

CLASSES="$WORK_DIR/classes"
STUBCLASSES="$WORK_DIR/stubclasses"
mkdir -p "$CLASSES" "$STUBCLASSES" "$(dirname "$OUTPUT")"

javac -nowarn --patch-module "jdk.jsobject=$STUB" \
  -d "$STUBCLASSES" "$STUB/netscape/javascript/"*.java
javac -nowarn --patch-module "jdk.jsobject=$STUB" \
  -d "$CLASSES" -cp "$BC" -sourcepath "$RS:$INJ" "$HERE/src/Dumper.java"

java -cp "$CLASSES" Dumper > "$OUTPUT"

python3 - "$OUTPUT" <<'PY'
from hashlib import sha256
from pathlib import Path
import sys
path = Path(sys.argv[1])
data = path.read_bytes()
print(f"candidate={path}")
print(f"sha256={sha256(data).hexdigest()}")
print(f"bytes={len(data)}")
PY
