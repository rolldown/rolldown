# Direct-Rolldown Controlled Vue Scale Curve

This fixture maps the fresh-process crossover of one ordinary Vue transform instance and the current Rolldown ParallelPlugin worker pool across nested selections ending at 5,000 real, content-unique Vue SFCs. The admitted pool contains 5,650 sources; the curve deliberately freezes a 5,000-source maximum. It invokes Rolldown directly and does not run the Vite build pipeline; unplugin-vue still calls Vite's synchronous Oxc helper internally for its TypeScript tail. It does not use duplicate modules, artificial delay, filter misses as scale, watch, rebuild, HMR, a development server, or cross-build worker reuse.

This is a prepared wide transform curve, not a representative Vue project graph. Imports emitted or retained by each compiled SFC are externalized after that SFC transform so dependency resolution cannot change the controlled source set. The separate independent-project requirement is intentionally outside this directory.

## Frozen corpus

`corpus-manifest.json` schema 2 pins five MIT repositories, commits, license paths and hashes, every retained source path and content hash, eligibility, exact duplicate removal, ordinary compile admission exclusions, nested ordering, summaries, and frozen selection hashes. This versioned amendment supersedes the earlier 4,540-source schema 1 curve.

| Repository       | Commit                                     | License SHA-256                                                    | Retained SFCs |     Bytes |
| ---------------- | ------------------------------------------ | ------------------------------------------------------------------ | ------------: | --------: |
| PrimeVue         | `d4374cb7c1267f35eba7cee5d0a266f50ca8ec84` | `39a2ce8d759cfcb59eccc49b0a417ad5c943f960c1bcdfba4720ca7547029af7` |         2,495 | 8,511,875 |
| Element Plus     | `85bdf740c1d550f3ca44472262e2a314039eab7d` | `0790118bb4d66681db1d63181f72ef68e632d632f6db0373ef87cf328561af27` |           725 | 1,942,309 |
| TDesign Vue Next | `dd334e2dc06d8ab48d1b6ebc5e9d4f6de67b16a2` | `b3dbcb89dcf4a11abf1b70d043795a3da0c458af16fefd2ff315d9ff5875312f` |           644 |   897,120 |
| Vuestic UI       | `c5337ed8e7e24ea294221326fe2ca6af8d3b8e1b` | `c44258bd026d8749142ac1b2cf0309f0b52655b3181c5ee4bfb6bd89103ab370` |           676 |   882,094 |
| Quasar           | `2165ce9f69d84e6169e7ca8a1c51fde105042cb9` | `830424149e83c3b9caa4243c36e73ac1b024b501fea99f8a22138b86eedc8d47` |         1,110 | 2,244,448 |

Eligibility uses `@vue/compiler-sfc` 3.5.39. Parse must succeed; style and custom blocks are excluded; any template, script, script-setup, style, or custom block using `src` is excluded; optional template preprocessors are excluded. Exact source SHA-256 duplicates retain the first UTF-8-sorted `repository/path`. The three committed Quasar path exclusions are the complete failure set from the pinned ordinary compile preflight: their tracked tsconfig extends missing generated `.quasar/tsconfig.json`. One duplicate of an excluded path is retained under a different tracked path and compiles in that context. The resulting pool of 5,650 distinct contents contains 14,477,846 bytes: 2,505 script-setup, 2,247 ordinary-script, and 898 template-only SFCs. The aggregate SHA-256 is `114f8b7b7e3fa7d13d5f14946acd7a4a42d88957f7ca57da041381cd6eada99c`.

Nested order is `SHA-256(aggregateSha256 + NUL + sourceKey)`, with UTF-8 source key as a tie-breaker. The committed prefixes are:

| Scale | Selection SHA-256                                                  |
| ----: | ------------------------------------------------------------------ |
|    32 | `542c27dc121c69009a27ebb77a75e2a5b8660b4e2c85ad3949c766af8ca59998` |
|   128 | `1a1833a66bd645d2f63886493dc0749ad05de6728549b6bb8af62a1fc7ff3591` |
|   256 | `0609df9cb9e6153bbd5a19325a7c82d17b4ec52f35c509f99bff94e67411100a` |
|   512 | `6770cadb2c52ae19ad3776e969d204b9f458be1e26f8e6d28d4a463001274d93` |
| 1,024 | `5d01c401de0e559934961478783dcb36ca9ddaa98fc6bc987a62e81726fe7b34` |
| 2,048 | `2483b221836c7f86610095ddab18f9f7ca42e22d857558347f8f8f3cffbcfed9` |
| 4,096 | `ffdfac9f785e570f8db341ce2afc1e66c40db8008e19c953e9bfc41e5829645f` |
| 5,000 | `27add878d7150bf40b5efc3540f0e78e029a6d4b076aae8914ba2b2ca7d6e474` |

## Preparation

Use exact clean detached checkouts named `primevue`, `element-plus`, `tdesign-vue-next`, `vuestic-ui`, and `quasar` under one source directory, or pass each checkout explicitly. Preparation verifies every checkout HEAD, license hash, eligibility decision, source hash, aggregate, and frozen prefix before atomically writing the ignored snapshot. Because compiler-sfc resolves imported macro types and unplugin-vue reads project tsconfig files, the snapshot copies every tracked file from each pin and 15 committed generated support entries; only the selected SFCs count as workload. Matrix startup rehashes the complete prepared support manifest and rejects missing, changed, or extra files. [SUPPORT.md](./SUPPORT.md) records the exact locks, tool commands, registry integrities, licenses, generated hashes, and preparation boundary.

| Repository       | Tracked support files | Support bytes | Support aggregate SHA-256                                          |
| ---------------- | --------------------: | ------------: | ------------------------------------------------------------------ |
| PrimeVue         |                 4,842 |    30,893,180 | `9ae0ef9efd81496401e282009b0b4a0f96699cff6e363850de3c1fe294589a06` |
| Element Plus     |                 2,695 |    10,262,230 | `02701c42251bfdb9e4733eb1fa15a86c3e1144e209b208755770f4dabcb09285` |
| TDesign Vue Next |                 2,432 |    19,573,898 | `562a21512c5ccc44efb8ff1fc4c555a642611ba24969912f337215bc4771f8ae` |
| Vuestic UI       |                 2,771 |    18,508,428 | `7c165fff3b07902c5932eceb30af1bca8a4d0af4706aa0b5222d5b830ea9784c` |
| Quasar           |                 3,759 |    62,665,447 | `a4356ae3969a2e6195c21a5ba01fb5163f0b87e88c1d0dadb9d60e2bbcc3d834` |

```sh
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./prepare-corpus.mjs --sources /path/to/vue-scale-sources
```

`--update-manifest` is only for reproducing the already frozen manifest from those exact commits. Any source, eligibility, ordering, scale, or expected-hash change requires a versioned protocol amendment before new timing.

## Runtime and allocation provenance

All runners reject active CI markers and any Node version other than 24.18.0. Runtime package resolution is explicit: the optional fourth `run-matrix.mjs` argument identifies the exact `packages/rolldown` root, and a process loader maps `rolldown`, `rolldown/experimental`, and `rolldown/parallelPlugin` in the main process and workers to that distribution. The runner verifies its clean repository commit, native binding hash, complete distribution hash, and stability across the matrix.

Commit `0aa600b5721b852cdc4095c7122a929a8cb4a798`, binding `deec0b2cb7a12e507ff223e12535c3280ab5fe8371f2fcc92f9db206163f1c5d`, and distribution `e30311e764bae7fba9afe27665db741d556a7c3728eb67cfbe7ce0fed3135ebc` are retained as the historical pre-protocol pin. They cannot be the formal wall baseline because that runtime calls `Worker.unref()` in the parent and can let a direct build exit with code 13 when no unrelated handle keeps the process alive. Correctness and wall matrices pin lifecycle-corrected commit `b144106882fe244b19b738fc0acf3ffa07c7c9f3`, release binding `7b8863bb28aefd2e2eb7409f8be6dae57a252fe4a2688383007be7ea2f847bf7`, and 17,095,091-byte distribution `1efffd0b63483e77cd2854fe716941000ae9548768691d7b5a64dceb011f3c45`; its only runtime change is the parent-ref lifecycle repair. Attribution pins research commit `8e35a2249b60b65120a44d1d896eeeed19dc703b`, release binding `6b7dfa175754ac57650768a68d7a567c5c0635a1bb47d47c5287914594c9795e`, and 17,140,783-byte distribution `68f57be9a8883a4ca6f28b57a9bac6e16907d8c1d079686ab9921b407b132735`. Metrics-off runs from that artifact must not be labelled unchanged-runtime wall evidence.

Every child explicitly receives `ROLLDOWN_WORKER_THREADS=18`, `RAYON_NUM_THREADS=12`, and `ROLLDOWN_MAX_BLOCKING_THREADS=4`. These are the configured Tokio, Rayon, and blocking capacities. They are separate from the JavaScript worker count, are not an OS CPU quota, do not report active CPU use, and must not be added as if every configured thread runs simultaneously.

Formal wall children are admitted only on macOS, AC power, low-power mode off, no recorded thermal or performance warning, at most 24 hours of uptime, at most 512 MiB starting swap, at most 2.0 one-minute load, at most 150% summed pre-child process CPU, and at least 50% free memory. Transient gates wait in ten-second intervals for at most five minutes. Every measured child must have zero pageout and swapout delta, and power, low-power, thermal, uptime, and swap gates are rechecked after the child so a final-run state change cannot be retained. All entrypoints reject an inherited non-empty `NODE_OPTIONS`; children receive only the exact research loader option, and reports record it. The known pre-restart host fails uptime and swap gates, so the runner must abort rather than emit wall evidence.

## Correctness and instrumentation

The generated entry re-exports every selected absolute SFC with tree shaking disabled. Final generation enables source maps. Every case compares raw and path-normalized code and map hashes, bytes, chunks, assets, and exports across variants.

The untimed smoke adds a coordinator audit hook and the Vue metrics buffer. It requires the exact selected IDs, one arrival and one Vue handler call per source, exact input bytes, every requested worker to handle at least one selected transform, exactly three constant wrapper filter misses, complete worker factories and lifecycle, clean Rust queue/permit state, clean termination, and identical ordinary/worker output. The smoke does not invoke `/usr/bin/time` and omits elapsed, CPU, and RSS fields. Internal instrumentation objects may contain incidental durations; they remain correctness-only and cannot enter a performance claim. Every report embeds a complete source manifest of this directory and `parallel-vue-plugin`; `create-correctness-evidence.mjs` binds a compact evidence pointer to the raw report SHA-256 and rejects a later harness edit.

Instrumented cases additionally allocate a Vue-only shared timeline with one fixed slot per frozen source ordinal. Each Vue kernel writes its worker number and `process.hrtime.bigint()` start/end timestamps. This is a nanosecond monotonic clock shared by worker threads in the same Node process, so records can reconstruct worker busy intervals, idle gaps, completion order, and the tail. The run brackets that clock with `Date.now()` before plugin setup and after the build, recording both epoch bounds, midpoint, bracket width, and uncertainty so it can be aligned with lifecycle epoch milliseconds and Rust anchors. The attribution gate requires one binding-module initialization record per process, complete main/process/worker CPU, heap, ELU, and GC snapshots, complete Rust arrival/acquire/complete events, and exact selected-ID alignment. Rust permit worker indices are registration slots and need not equal JavaScript `threadNumber`; the gate proves and records a stable bijection from permit index to worker thread number using per-module Rust and JavaScript events. `attribution-contract-smoke-matrix.json` exercises that contract on 32 sources without collecting wall evidence. Instrumented child capture explicitly allows 64 MiB and treats `spawnSync` errors such as `ENOBUFS` as harness failures. Wall cases set `instrumentation: false`: they do not construct the source-to-ordinal map, shared timeline, clock anchors, metrics buffer, coordinator audit, or Rust metrics.

The existing transform-only adapter compiles with production inline templates and covers SFC parse, script setup, TypeScript, templates, code generation, imports, JIT, and compiler errors. A coordinator-only plugin supplies unplugin-vue's single export-helper virtual module equally to ordinary and worker variants; other imports emitted after each SFC transform remain external. The adapter sets unplugin-vue `sourceMap: false`; the correctness gate proves deterministic equality of the final Rolldown-generated maps, not original-SFC mapping fidelity or a complete transform-level source-map chain. The adapter still excludes styles, custom and external blocks, other child virtual modules, full warning/diagnostic parity, non-cloneable options, watch lifecycle, rebuild, and HMR. This curve is mechanical and resource evidence, not product crossover evidence.

### Current untimed development result

An untimed 5,000-source ordinary compile preflight completed with zero errors. The untimed correctness smoke then completed ordinary, worker-1, worker-4, and worker-8 against the lifecycle-corrected baseline. Every variant saw exactly 5,000 selected IDs and 12,970,626 input bytes once, produced identical normalized code hash `ed3301a5a9391e4b9fc5698c408664dfacbaf5c90ebd324e7d6d99ee9e45593f` and map hash `c4269cfb0d904ca08411122f4e2dea352f13adbce2f545ed715cb6db94754aa9`, exercised every requested worker, and passed lifecycle plus Rust queue/permit cleanup validation. Those ignored development artifacts predate the final source-manifest evidence gate and are not the durable admission for formal timing. Before timing, `run-admission-audit.mjs` must prove that the complete content-deduplicated 1,112-source, 2,245,255-byte pre-exclusion Quasar candidate set with aggregate `49f5089abac134b76c7e9ee6e21db1c073ebcdf85e1cd46d65c3ef82fe36945d` fails only at the three frozen paths and that all 5,650 retained sources compile and generate; the clean committed harness must then rerun the correctness smoke and create its compact evidence pointer.

## Matrix order

Build or restore the exact pinned release artifacts before running any matrix.

```sh
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./wall-screen-matrix.json /dev/null /path/to/lifecycle-corrected/packages/rolldown --validate-only
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./instrumented-matrix.json /dev/null /path/to/instrumented/packages/rolldown --validate-only
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-admission-audit.mjs /dev/null /dev/null /path/to/lifecycle-corrected/packages/rolldown --validate-only
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-admission-audit.mjs ./evidence/raw/admission.json ./evidence/admission.json /path/to/lifecycle-corrected/packages/rolldown
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./verify-admission-evidence.mjs ./evidence/raw/admission.json ./evidence/admission.json
git add ./evidence/raw/admission.json ./evidence/admission.json
git commit -m "research: admit controlled Vue corpus"
test -z "$(git status --short)"
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./compile-preflight.mjs 5000 ./.results/preflight-5000.json /path/to/lifecycle-corrected/packages/rolldown
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./smoke-matrix.json ./evidence/raw/correctness.json /path/to/lifecycle-corrected/packages/rolldown
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./create-correctness-evidence.mjs ./evidence/raw/correctness.json ./evidence/correctness.json
git add ./evidence/raw/correctness.json ./evidence/correctness.json
git commit -m "research: admit controlled Vue correctness"
test -z "$(git status --short)"
test "$(git ls-files ./evidence/admission.json ./evidence/correctness.json ./evidence/raw/admission.json ./evidence/raw/correctness.json | wc -l | tr -d ' ')" = 4
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./attribution-contract-smoke-matrix.json ./.results/attribution-contract.json /path/to/instrumented/packages/rolldown
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./wall-screen-matrix.json ./.results/wall-screen.json /path/to/lifecycle-corrected/packages/rolldown
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./create-confirm-matrix.mjs ./.results/wall-screen.json ./.results/wall-confirm-matrix.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./.results/wall-confirm-matrix.json ./.results/wall-confirm.json /path/to/lifecycle-corrected/packages/rolldown
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./summarize-matrix.mjs ./.results/wall-confirm.json ./.results/wall-confirm-summary.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./write-additional-confirm-matrix.mjs ./.results/wall-confirm-summary.json ./.results/wall-additional-matrix.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./.results/wall-additional-matrix.json ./.results/wall-additional.json /path/to/lifecycle-corrected/packages/rolldown
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./merge-confirmation-reports.mjs ./.results/wall-confirm.json ./.results/wall-additional.json ./.results/wall-confirm-merged.json
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs ./instrumented-matrix.json ./.results/instrumented.json /path/to/instrumented/packages/rolldown
```

`run-admission-audit.mjs` is the eligibility admission: it runs the content-deduplicated pre-exclusion Quasar set and the complete retained pool in fresh ordinary processes, embeds runtime, harness, package-lock, and resolved compiler provenance, writes raw evidence plus a compact SHA-256 pointer, and never collects performance fields. `smoke-matrix.json` records an ordinary golden at every frozen scale plus worker-1, worker-4, and worker-8 parity at 5,000 sources. Both evidence runs require a clean fixture and clean pinned runtime, so the admission raw report and pointer must be committed before starting correctness. After correctness, its raw report and pointer must also be committed and the worktree must be clean before any attribution or wall runner continues. Their raw reports and compact pointers live under `evidence/`; formal runners require all four files to be tracked and byte-identical to `HEAD`, and reject evidence that only exists in ignored local state. The committed correctness evidence retains raw and path-normalized code/map hashes; formal runs require portable normalized hashes, bytes, chunks, assets, and exports to match those goldens and independently require raw equality among variants executed from the same checkout. `compile-preflight.mjs` remains a focused frozen-prefix diagnosis. `wall-screen-matrix.json` contains one fresh uninstrumented pass of ordinary plus every worker count from one through eight at all eight frozen scales, with scale-level order rotation and no discarded process. A screen only selects work, and a positive-to-negative reversal aborts rather than choosing a favorable interval.

`create-confirm-matrix.mjs` pins the source screen report hash and generates the repeated matrix after the first direction change is known. It selects the lower endpoint, candidate, next larger point, and full corpus; at each scale it includes ordinary, the screened best worker, adjacent eligible counts, and the fixed policy candidates worker-4 and worker-8, with duplicates removed. Runs below two seconds receive 15 rotated blocks and longer runs receive 10. A screen with no positive base point repeats the two largest boundaries; a positive smallest point is recorded as left-censored. A non-monotonic pattern that cannot support the frozen selection rule aborts for inspection instead of silently choosing favorable points.

`summarize-matrix.mjs` pairs every worker sample with ordinary execution from the same rotated block, computes paired medians and 100,000 deterministic percentile-bootstrap resamples with seed `0x20260712`, and applies the smaller-count overlap/under-two-percent tie rule against the fixed fastest candidate. Mechanical selection and resource selection are separate: the resource optimum is the fastest tie-adjusted member of the workers that already pass every frozen wall, CPU, RSS, absolute-memory, output, and paging gate. A crossover is exact only when the immediately smaller frozen scale is repeated negative and the candidate plus its actual immediately larger frozen scale are repeated positive. The summary reports left/right censoring, inconsistent repeated direction, or an executable additional-confirmation matrix instead of converting an incomplete boundary into an exact crossover. Additional reports are pinned to and merged with the prior report before the next summary iteration. The summary also exposes a stable `/policyEvidence/byScale/<scale>/variants/<variant>` object for every repeated variant. Each object contains the wall, total-CPU, and peak-RSS medians, resource eligibility, the paired worker/ordinary wall-ratio bootstrap upper bound, and the tie-adjusted resource-eligible oracle count so a fixed-count policy evaluator can bind exact JSON Pointers without reinterpreting the statistical report. Ordinary is resource-eligible and has oracle count zero when no worker passes the resource gates.

`instrumented-matrix.json` is attribution-only and covers every base scale and fixed count once. It is deliberately separate from uninstrumented wall evidence. Repeated attribution can later narrow to confirmed crossover neighbors and full scale without changing the frozen source selections.
