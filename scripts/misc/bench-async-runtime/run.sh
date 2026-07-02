#!/usr/bin/env bash
# A/B the stock tokio binding vs the shared-async-runtime binding on
# rolldown-benchmark fixtures. See README.md for prerequisites and methodology.
#
# Usage: ./run.sh /abs/path/to/rolldown-benchmark apps/1000 [apps/10000 ...]
set -euo pipefail

BENCH_ROOT=$1; shift
DIR=$(cd "$(dirname "$0")" && pwd)
OUT="$DIR/results-$(date +%Y%m%d-%H%M%S)"

TOKIO=/tmp/bench-tokio.node
SHARED=/tmp/bench-shared.node
GLUE="$DIR/../../../packages/rolldown/dist/index.mjs"

for f in "$TOKIO" "$SHARED"; do
  [ -f "$f" ] || { echo "missing $f — build it first (see README.md)" >&2; exit 1; }
done
[ -f "$GLUE" ] || { echo "missing $GLUE — run: pnpm --filter rolldown build-js-glue" >&2; exit 1; }

mkdir -p "$OUT"
{
  echo "date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "commit: $(git -C "$DIR" rev-parse HEAD)"
  echo "node: $(node --version)"
  echo "tokio_binding: $(ls -l "$TOKIO" | awk '{print $5}') bytes"
  echo "shared_binding: $(ls -l "$SHARED" | awk '{print $5}') bytes"
} > "$OUT/meta.txt"

for FIX in "$@"; do
  F="$BENCH_ROOT/$FIX"
  [ -d "$F" ] || { echo "no such fixture dir: $F" >&2; exit 1; }
  TAG="${FIX//\//_}"

  # (a) wall time — hyperfine runs each command's warmups+runs sequentially;
  # accepted methodology here, warmups absorb cache effects.
  hyperfine --warmup 3 --runs 12 \
    --export-json "$OUT/$TAG-wall.json" \
    --export-markdown "$OUT/$TAG-wall.md" \
    -n tokio  "NAPI_RS_NATIVE_LIBRARY_PATH=$TOKIO  FIXTURE=$F node $DIR/direct.mjs" \
    -n shared "NAPI_RS_NATIVE_LIBRARY_PATH=$SHARED FIXTURE=$F node $DIR/direct.mjs"

  # (b) instructions retired, max RSS, ctx switches — 3 samples per side
  # (/usr/bin/time -l is macOS-specific).
  for side in tokio shared; do
    for i in 1 2 3; do
      /usr/bin/time -l env NAPI_RS_NATIVE_LIBRARY_PATH=/tmp/bench-$side.node FIXTURE="$F" \
        node "$DIR/direct.mjs" 2>> "$OUT/$TAG-$side-time.txt" >/dev/null
    done
  done

  # (c) peak thread count — 50ms `ps -M` sampler while one build runs
  for side in tokio shared; do
    NAPI_RS_NATIVE_LIBRARY_PATH=/tmp/bench-$side.node FIXTURE="$F" \
      node "$DIR/direct.mjs" >/dev/null & pid=$!
    max=0
    while kill -0 $pid 2>/dev/null; do
      # The child may exit between `kill -0` and `ps`; under pipefail a bare
      # assignment from the failed pipeline would abort the whole script, so
      # treat a failed sample as "child gone, loop done".
      if n=$(ps -M $pid 2>/dev/null | tail -n +2 | wc -l | tr -d ' '); then
        [ "$n" -gt "$max" ] && max=$n
      else
        break
      fi
      sleep 0.05
    done
    wait $pid || { echo "$side direct.mjs failed for $FIX" >&2; exit 1; }
    echo "$side peak_threads=$max" >> "$OUT/$TAG-threads.txt"
  done
done

echo "results in $OUT"
