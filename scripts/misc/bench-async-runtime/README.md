# bench-async-runtime

A/B benchmark harness for two prebuilt rolldown bindings, measured on
[`rolldown-benchmark`](https://github.com/rolldown/rolldown-benchmark) fixtures.

It was written for **tokio vs shared-async-runtime**, and the recorded
comparison (`internal-docs/async-runtime/benchmarks.md`) dates from when the
binding still had a tokio flavor. The binding-level tokio runtime was removed
(default flipped in `28383e8a3`, feature deleted in `4a9ca6b81`), so **building
from this tree can only produce shared-runtime binaries** — a tokio baseline
must be built from a pinned tokio-era revision (below). The harness itself is
flavor-agnostic: it compares whatever two `.node` files you point it at.

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
- two prebuilt release bindings to compare.

To compare two shared-runtime builds (e.g. two commits of this branch), build
each and copy it aside:

```bash
pnpm --filter rolldown build-binding --release
cp packages/rolldown/src/rolldown-binding.darwin-arm64.node /tmp/bench-shared.node
pnpm --filter rolldown build-js-glue
```

To reproduce the **historical tokio-vs-shared** comparison, the tokio side must
come from a tokio-era revision (the last one is `4a9ca6b81~1`; the recorded
run used the era of `21ae121b2`, where `default = ["tokio-runtime"]`):

```bash
git worktree add /tmp/rolldown-tokio-era 4a9ca6b81~1
cd /tmp/rolldown-tokio-era && pnpm install
pnpm --filter rolldown build-binding --release   # default features = tokio-runtime
cp packages/rolldown/src/rolldown-binding.darwin-arm64.node /tmp/bench-tokio.node
```

Cross-commit caveat: the JS glue and the binding ABI move together — run each
side's build through its **own** commit's glue, or keep to fixtures that avoid
the drifted surface. The committed 2026-07-02 numbers were same-commit builds
and are not subject to this caveat.

Sanity: a release binding is ~16 MB (`strip = "symbols"`); if you see ~96 MB
you copied a stale **debug** binding — rebuild.

## Usage

```bash
scripts/misc/bench-async-runtime/run.sh ~/workspace/github/rolldown-benchmark apps/1000 apps/10000
```

Results land in `scripts/misc/bench-async-runtime/results-<timestamp>/`
(gitignored):

| file                          | contents                                                                         |
| ----------------------------- | -------------------------------------------------------------------------------- |
| `meta.txt`                    | commit, node version, binding sizes                                              |
| `<fixture>-wall.json` / `.md` | hyperfine wall-time stats (JSON + markdown)                                      |
| `<fixture>-<side>-time.txt`   | `/usr/bin/time -l`: instructions retired, max RSS, ctx switches (3 samples/side) |
| `<fixture>-threads.txt`       | peak thread count per side                                                       |

## Methodology notes

- **Wall time**: hyperfine, 3 warmups + 12 runs per side. Hyperfine runs each
  command's runs sequentially (not interleaved); the warmups absorb
  first-run-after-copy and cache effects. Machine noise floor here is
  ~4–7 ms stdev — treat differences within that as noise.
- **Counters**: `/usr/bin/time -l` (macOS) reports `instructions retired`,
  `maximum resident set size`, and voluntary/involuntary context switches.
- **Threads**: `ps -M <pid>` sampled every 50 ms while one build runs; the
  maximum row count is the peak thread count. In the historical tokio-vs-shared
  setup, tokio (multi-threaded runtime + blocking pool) peaked well above the
  shared runtime (~30+ vs ~15 on `apps/1000`), so shared >= tokio meant the env
  var was not reaching the child process. When both sides are shared-runtime
  builds, near-identical peaks are expected and prove nothing about the env
  var — verify binding selection via `meta.txt` binding sizes/paths instead.
- Keep the machine otherwise idle; each process is a fresh Node instance, so
  JIT warmup is included on both sides equally.
