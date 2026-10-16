#!/bin/sh
# Requires Python 3.11+, pinned source and JDK from sources.json. --check is read-only.
set -eu
HERE=$(cd "$(dirname "$0")" && pwd)
exec /opt/homebrew/bin/python3 "$HERE/regenerate.py" "$@"
