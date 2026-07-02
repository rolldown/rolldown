# bench-async-runtime

A/B benchmark harness: stock **tokio** binding vs the **shared-async-runtime**
binding (`--features async-runtime`), measured on
[`rolldown-benchmark`](https://github.com/rolldown/rolldown-benchmark) fixtures.

## How binding selection works

`packages/rolldown/src/binding.cjs` (and the generated dist glue) honors
`NAPI_RS_NATIVE_LIBRARY_PATH` **first**, before platform detection. Each
benchmarked process points that env var at one of two prebuilt `.node` files —
no file swapping, no branch switching between runs.

`direct.mjs` runs one build **in-process** (no CLI fork) so `/usr/bin/time`,
`ps`, and profilers observe the actual work.

## Prerequisites

- macOS (uses `/usr/bin/time -l` and `ps -M`), `hyperfine` >= 1.20, Node >= 20.11
- a checkout of `rolldown-benchmark` with dependencies installed
- both bindings and the JS glue built **from the same commit**:

```bash
pnpm --filter rolldown build-binding --release
cp packages/rolldown/src/rolldown-binding.darwin-arm64.node /tmp/bench-tokio.node
pnpm --filter rolldown build-binding --release --no-default-features --features async-runtime
cp packages/rolldown/src/rolldown-binding.darwin-arm64.node /tmp/bench-shared.node
pnpm --filter rolldown build-js-glue
```

Sanity: a release binding is ~16 MB (`strip = "symbols"`); if you see ~96 MB
you copied a stale **debug** binding — rebuild.

## Usage

```bash
scripts/misc/bench-async-runtime/run.sh ~/workspace/github/rolldown-benchmark apps/1000 apps/10000
```

Results land in `scripts/misc/bench-async-runtime/results-<timestamp>/`
(gitignored):

| file | contents |
| --- | --- |
| `meta.txt` | commit, node version, binding sizes |
| `<fixture>-wall.json` / `.md` | hyperfine wall-time stats (JSON + markdown) |
| `<fixture>-<side>-time.txt` | `/usr/bin/time -l`: instructions retired, max RSS, ctx switches (3 samples/side) |
| `<fixture>-threads.txt` | peak thread count per side |

## Methodology notes

- **Wall time**: hyperfine, 3 warmups + 12 runs per side. Hyperfine runs each
  command's runs sequentially (not interleaved); the warmups absorb
  first-run-after-copy and cache effects. Machine noise floor here is
  ~4–7 ms stdev — treat differences within that as noise.
- **Counters**: `/usr/bin/time -l` (macOS) reports `instructions retired`,
  `maximum resident set size`, and voluntary/involuntary context switches.
- **Threads**: `ps -M <pid>` sampled every 50 ms while one build runs; the
  maximum row count is the peak thread count. Expect tokio (multi-threaded
  runtime + blocking pool) to peak well above the shared runtime (~30+ vs ~15
  on `apps/1000`); if shared >= tokio, the env var is probably not reaching
  the child process — investigate before trusting any numbers.
- Keep the machine otherwise idle; each process is a fresh Node instance, so
  JIT warmup is included on both sides equally.
