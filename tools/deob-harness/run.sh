#!/bin/sh
# Build + run the deob headless harness. Requires: javac/java on PATH.
# bcprov 1.52 is fetched once from Maven Central (compile-only; TLS classes
# are never loaded at harness runtime).
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
RS="/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/runescape-client/src/main/java"
INJ="/Users/tylercovalt/Documents/ChatGPT/RSPSi-resources/RuneLite-melxin/injection-annotations/src/main/java"
STUB="$HERE/stubs"
OUT="${1:-/tmp/deobwork}"
CLASSES="$OUT/classes"
STUBCLASSES="$OUT/stubclasses"
mkdir -p "$CLASSES" "$STUBCLASSES" "$OUT/libs"
BC="$OUT/libs/bcprov-jdk15on-1.52.jar"
if [ ! -f "$BC" ]; then
  curl -sL --max-time 120 -o "$BC" \
    https://repo1.maven.org/maven2/org/bouncycastle/bcprov-jdk15on/1.52/bcprov-jdk15on-1.52.jar
fi
javac -nowarn --patch-module "jdk.jsobject=$STUB" \
  -d "$STUBCLASSES" "$STUB/netscape/javascript/"*.java
javac -nowarn --patch-module "jdk.jsobject=$STUB" \
  -d "$CLASSES" -cp "$BC" -sourcepath "$RS:$INJ" "$HERE/src/Dumper.java"
FIX="$HERE/../../reference-fixtures/deob_golden.txt"
java -cp "$CLASSES" Dumper | tee "$FIX"
