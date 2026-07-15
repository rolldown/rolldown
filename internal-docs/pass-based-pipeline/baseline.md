# Link Pipeline Baseline

This document records the behavior and cost baseline used to judge the pass-based link migration. It is a reproducibility contract, not a general benchmark claim: comparisons are valid only on the same clean commit lineage, machine, affinity, toolchain, profile, allocator, workload image, and runner schema.

## Measured boundaries

`link-time` scans a fresh `Bundle` outside the timer and measures exactly `LinkStage::new(scan_stage_output, &options).link()`. The three returned values remain live until after `Instant::elapsed`; the post-timer `black_box`, module-count read, diagnostic observation, and drops are excluded.

`bundle-time` measures a fresh standard `Bundle::generate()` over the already populated in-memory filesystem. The returned `BundleOutput` stays live until the clock is read and is passed to `black_box` only after timing stops. It is a full scan, link, and Generate control and is not used as a substitute for link-only time.

`link-trace` uses the same scan-then-link boundary with one process-global, span-only collector installed before the runtime starts. The collector is reset between warmups and samples, reaches already-created Rayon workers, ignores all events and `TRACE` metadata work, and uses collector-owned stable span identities rather than reusable registry IDs. Trace values are descriptive attribution with collector overhead; they are never substituted for `link-time`. A sample is rejected unless it contains exactly one entered link span. Direct-child coverage is the union of enter/exit intervals clipped to that span. `direct_children_inclusive_sum_ns = direct_children_wall_coverage_ns + direct_children_overlap_excess_ns` and `link_span_ns = direct_children_wall_coverage_ns + inside_link_unattributed_ns`. Every `rolldown::pass` span overlapping link must name the link span as its direct parent; any detached pass rejects the run.

`digest` runs the ordinary `Bundle::generate()` path inside a testing-only Tokio task scope. A link-boundary observer borrows the ordered mixed-severity diagnostics from the produced `LinkStageOutput`, restores the same accumulator, and then lets the unchanged standard Generate path continue. A missing observer result rejects the run rather than emitting a report with a false capture-model label. The default `bundle_up` source is untouched. The report hashes semantic code and data exactly, encodes typed paths as separate `cwd`, `cwd-relative`, or `literal` frames, preserves every observable vector order, and records success and failure as explicit outcomes.

`link-rss` reports the GNU-time peak of the whole process that constructs the pinned in-memory input, scans it, and links it. `scan-rss` is the corresponding construction-plus-scan control. These independent maxima are compared separately and are never subtracted or described as pure link allocation.

## Versioned workload images

Runner report schema is version 4, workload generator schema is version 2, and digest schema is `rolldown-link-baseline-digest-v4`. Synthetic inputs use seed `0x6c696e6b5f763031`. Every digest uses length-prefixed XXH3-128 framing. The v4 report marks every run as `canonical: true` or `canonical: false` and carries embedded build provenance; non-canonical development output cannot be mistaken for accepted baseline evidence.

| Workload           | Source files | Source bytes | Input digest                       | Linked modules including runtime |
| ------------------ | -----------: | -----------: | ---------------------------------- | -------------------------------: |
| `overhead-64`      |           64 |        8,376 | `78c70e0cae50f65611eb18e2778e2791` |                               65 |
| `wide-4096`        |        4,096 |      542,191 | `a4352d87b0afdb449f386c9f4f94bae1` |                            4,097 |
| `deep-1024`        |        1,024 |      136,484 | `1b408cfbea976792a83bc4419863ff42` |                            1,025 |
| `scc-256x4`        |        1,025 |      116,358 | `146e457a611d041edcb86a7b81d2de2b` |                            1,026 |
| `export-star-1024` |        2,048 |      117,171 | `2559380bc3588fa5ae719a2a31a6734d` |                            2,049 |
| `cjs-2048`         |        2,049 |      211,769 | `d59a17f985d639bdf05c5d911cd5cd0f` |                            2,050 |
| `json-2048`        |        2,049 |      222,009 | `127f8a9a04531df66c6ac20518c54051` |                            2,050 |
| `dynamic-1024`     |        1,025 |       79,326 | `cf3c6f6754d17f9bb59018887db85c50` |                            1,026 |
| `three-r108`       |          610 |    1,474,106 | `d3c715c37ba5df677fe7e530088a4487` |                              371 |
| `rome`             |        9,041 |   15,108,932 | `771e707bc478f9316712dfb7647f4422` |                            1,215 |

Three is pinned to `7e0a78beb9317e580d7fa4da9b5b12be051c6feb` and loads only transformed `entry.js` plus `src/`; unrelated examples, media, builds, docs, and Git data are excluded. Rome is pinned to `d95a3a7aab90773c9b36d9c82a08c8c4c6b68aa5` and loads transformed `src/`. The runner rejects a source HEAD mismatch or any transformed file-count, byte-count, or digest drift.

`diagnostic-order` is digest-only: 12 files, 909 bytes, input digest `9586764cf0f88b8c207e13aaf4b452d9`, and 13 linked modules including runtime. It contains two independent circular-dependency warnings, two require-TLA errors, and two missing-export errors. The immutable baseline stores independent cycles in an `FxHashSet`, so it does not define their cross-cycle order: 39 of 40 formal processes emitted B then A and one emitted A then B. Each cycle descriptor and its internal path are stable, and the following four errors are byte-for-byte stable and ordered. `ComputeModuleExecutionOrderPass` intentionally replaces only the cycle container with a first-discovery `FxIndexSet`; candidate comparisons therefore require the exact full link-owned order A cycle, B cycle, two require-TLA errors, and two missing-export errors.

## Canonical environment

| Property      | Value                                                                                         |
| ------------- | --------------------------------------------------------------------------------------------- |
| Machine       | Intel Core i5-13500H, Linux x86_64                                                            |
| CPU affinity  | `0,2,4,6`, one logical CPU from each P-core                                                   |
| CPU governor  | `performance` on all four pinned CPUs                                                         |
| Timing policy | Pinned CPUs fixed at 2.6 GHz for each accepted link-time window, then restored to 0.4–4.7 GHz |
| Rayon         | `RAYON_NUM_THREADS=4` for time, trace, and RSS; both `1` and `4` for digest                   |
| Rust          | `rustc 1.97.0 (2d8144b7880597b6e6d3dfd63a9a9efae3f533d3)`; LLVM 22.1.6                        |
| Cargo         | `cargo 1.97.0 (c980f4866141969fab6254a680546a277789d6f0)`                                     |
| Node          | `v24.12.0`                                                                                    |
| Profile       | Cargo `release`, fat LTO, one codegen unit                                                    |
| Allocator     | `mimalloc`                                                                                    |

Canonical reports fail closed unless they record `canonical: true`, `git_dirty: false`, `build_profile: release`, `LC_ALL=C`, `cpus_allowed_list: 0,2,4,6`, `performance` for each pinned CPU, the mode-specific Rayon value, the exact Git HEAD, the exact Rust/Cargo/Node versions above, and the exact manifest. The executable must also contain verified build-time provenance from `just build-link-baseline`: clean build commit and tree, exact rustc and Cargo, release profile, optimization level 3, fat LTO, one codegen unit, stripped symbols, Linux x86_64 host and target, and no extra rustflags. The embedded commit and toolchain must match the runtime checkout and metadata, so an old or differently compiled binary cannot label itself canonical. RSS metadata must be non-empty, must match the current repository HEAD and exact toolchain, and must be captured in the parent process. The host must have no competing build or benchmark on the pinned CPUs. Load average is recorded for diagnosis but is not by itself an idle-host proof. `--development` explicitly produces `canonical: false` output and bypasses machine/profile pins for smoke tests; it is never accepted as Phase 1 evidence.

## Commands

Build the runner from a clean worktree:

```bash
unshare --mount sh -c 'mount --bind /tmp/codex-valid-null /dev/null && exec just build-link-baseline'
```

Canonical timing invocation:

```bash
taskset -c 0,2,4,6 env LC_ALL=C RAYON_NUM_THREADS=4 target/release/link-baseline --mode link-time --workload wide-4096 --warmups 10 --samples 50 --iterations-per-sample 32 --output tmp/link-baseline/phase1/<commit>/attempt-01/link-time/wide-4096.json
```

Digest runs use one fresh process per sample and repeat every workload, including `diagnostic-order`, twenty times with Rayon 1 and twenty times with Rayon 4:

```bash
taskset -c 0,2,4,6 env LC_ALL=C RAYON_NUM_THREADS=1 target/release/link-baseline --mode digest --workload diagnostic-order --warmups 0 --samples 1 --iterations-per-sample 1 --output tmp/link-baseline/phase1/<commit>/attempt-01/digest/diagnostic-order/rayon-1-01.json
```

RSS modes must not start metadata probes inside the GNU-time process. Capture the values in the parent shell, export them as `ROLLDOWN_LINK_BASELINE_GIT_COMMIT`, `ROLLDOWN_LINK_BASELINE_GIT_DIRTY`, `ROLLDOWN_LINK_BASELINE_RUSTC`, `ROLLDOWN_LINK_BASELINE_RUSTC_VERBOSE`, `ROLLDOWN_LINK_BASELINE_CARGO`, and `ROLLDOWN_LINK_BASELINE_NODE`, then time only the runner:

```bash
env LC_ALL=C RAYON_NUM_THREADS=4 ROLLDOWN_LINK_BASELINE_GIT_COMMIT="$COMMIT" ROLLDOWN_LINK_BASELINE_GIT_DIRTY=false ROLLDOWN_LINK_BASELINE_RUSTC="$RUSTC" ROLLDOWN_LINK_BASELINE_RUSTC_VERBOSE="$RUSTC_VERBOSE" ROLLDOWN_LINK_BASELINE_CARGO="$CARGO" ROLLDOWN_LINK_BASELINE_NODE="$NODE" /usr/bin/time -v -o "$OUT/rss/link-rss/wide-4096/01.time" taskset -c 0,2,4,6 target/release/link-baseline --mode link-rss --workload wide-4096 --warmups 0 --samples 1 --iterations-per-sample 1 --output "$OUT/rss/link-rss/wide-4096/01.json"
```

## Acceptance rules

- Reject any timing workload whose relative median absolute deviation exceeds 1%. Increase only that workload’s independent iterations per statistical sample and rerun it; do not widen the regression budget to absorb noise.
- Use 100 warmups and 500 statistical samples for `overhead-64`; use 10 warmups and 50 statistical samples for other time workloads. Trace uses 2 warmups and 10 descriptive samples with one iteration each.
- Require every trace sample to contain one singly-entered link span, no direct-child interval outside it, the two interval identities above, and an empty `detached_passes` list. A future parallel driver must propagate both the tracing dispatcher and an explicit link parent; re-entering the link span on workers is rejected.
- Set each link-time allowance to `max(3%, 4 × baseline relative MAD)` capped at 5%, and require the geometric mean across all ten workloads to regress by no more than 1%.
- For every digest workload, require one unique output value across all forty fresh processes and equality between Rayon 1 and Rayon 4. For link-owned diagnostic producers with a defined order, descriptor arrays must be identical so order and spans are not hidden behind hashes.
- Treat diagnostics already produced outside link according to the boundary being measured, not as an invented link ordering guarantee. Rome's forty `UNRESOLVED_IMPORT` warnings are appended by scan tasks in completion order; require one exact descriptor multiset across processes, while excluding only their cross-warning order from the link comparison. No missing, extra, changed, or respanned descriptor is allowed.
- For the immutable pre-extraction baseline `diagnostic-order` image, retain the exact cycle-warning multiset and exact ordered four-error suffix because that historical implementation does not define cross-cycle order. Every candidate containing `ComputeModuleExecutionOrderPass` must emit one exact full link-owned diagnostic array in first-discovery order across all fresh Rayon 1 and Rayon 4 processes.
- For each RSS mode and workload, use ten fresh processes. Compute the median, median absolute deviation, and nearest-rank p90 from GNU-time `Maximum resident set size (kbytes)`. Require relative MAD at most 1%; set the median allowance to `max(2%, 4 × relative MAD, 8 MiB / median)` capped at 5%, and the p90 allowance to `max(3%, 16 MiB / p90)`.
- Repeat a stable timing distribution that exceeds its regression allowance once under the same controlled conditions and classify a regression only after the repeated stable comparison fails. RSS retries are noise retries instead: accept the first paired attempt whose baseline and candidate relative MAD are both at most 1%, stop after six paired attempts, and classify a still-unstable cell as `blocked-noise` rather than inferring a regression from it.

## Rejected measurements

The first single-iteration attempt is preserved under ignored `tmp/link-baseline/baseline/`. Relative MAD ranged from 2.79% to 9.57%, so no value is accepted.

The first batched attempt is preserved under ignored `tmp/link-baseline/baseline-v2/`. `overhead-64` used 100 iterations per sample and measured 7.81% relative MAD; `cjs-2048` used 10 and measured 10.31%. An unrelated Astro build was simultaneously using about two pinned P-cores and roughly 10 GiB of memory, so the run was stopped and rejected rather than used to relax the gate.

The formal trace structure capture at `723c52d6f854e6ea065ecf7012d77c523e79f26a` contains 100 valid samples across ten workloads: every sample has exactly one link span, no detached pass, zero overlap in the serial driver, and both interval identities hold. Its duration attribution is rejected because the same competing Astro build drove relative MAD from 1.56% to 16.89%. A 64-iteration Rome link calibration averaged 8.541 ms under load and is retained only as a runtime estimate. No formal timing, bundle-time, or RSS distribution was started under that contention.

## Accepted results

The immutable pre-migration base is clean commit `723c52d6f854e6ea065ecf7012d77c523e79f26a`, tree `2d42688a9b3fa81b838de5f100a8248d96b08400`. Its schema-v4 runner was built with verified release provenance and the canonical environment above. Raw formal results live under `tmp/link-baseline/phase1/723c52d6f854e6ea065ecf7012d77c523e79f26a/` in the dedicated pinned worktree.

The accepted same-base candidate is clean draft-PR commit `9dc3e0b3fef8e486a13507e8cfd0640b2470b1e0`, tree `4da186be0450e0ebcc39efc686ea8c91fdbf7ef2`. Baseline reports selected after the immutable overhead control live under `tmp/link-baseline/phase1/723c52d6f854e6ea065ecf7012d77c523e79f26a/attempt-24-fixed-2.6ghz-full-matrix/` in the baseline worktree; the immutable overhead report is `attempt-23-fixed-2.6ghz/link-time/overhead-64.json`. Candidate reports, rejected attempts, symmetric preconditioning reports, and the final selection live under `tmp/link-baseline/candidate/9dc3e0b3fef8e486a13507e8cfd0640b2470b1e0/attempt-01-fixed-2.6ghz/` in the candidate worktree. The final comparison is `link-time-comparison.json`, SHA256 `fe838ba90228d60398ce42d93a79e3f777202da1f1b987329880667a94f5c76d`; its selected-report manifest is `link-time-selection.tsv`, SHA256 `2add848a1e92298d5c6e0e054b52924c22efc7a920df17ed51e615abb855b5d8`.

| Workload           | Base median (ns) | Base rMAD | Candidate median (ns) | Candidate rMAD | Change | Allowance | Accepted |
| ------------------ | ---------------: | --------: | --------------------: | -------------: | -----: | --------: | :------: |
| `overhead-64`      |          244,642 |    0.674% |               224,020 |         0.873% | -8.43% |     3.00% |   Yes    |
| `wide-4096`        |        7,312,931 |    0.932% |             7,246,594 |         0.722% | -0.91% |     3.73% |   Yes    |
| `deep-1024`        |        1,582,628 |    0.816% |             1,572,554 |         0.731% | -0.64% |     3.27% |   Yes    |
| `scc-256x4`        |        1,097,099 |    0.775% |             1,069,288 |         0.994% | -2.53% |     3.10% |   Yes    |
| `export-star-1024` |        4,870,298 |    0.784% |             4,870,897 |         0.994% | +0.01% |     3.13% |   Yes    |
| `cjs-2048`         |        6,222,876 |    0.645% |             6,041,184 |         0.747% | -2.92% |     3.00% |   Yes    |
| `json-2048`        |       13,192,757 |    0.441% |            12,490,001 |         0.424% | -5.33% |     3.00% |   Yes    |
| `dynamic-1024`     |        2,035,126 |    0.574% |             2,028,028 |         0.929% | -0.35% |     3.00% |   Yes    |
| `three-r108`       |        1,958,752 |    0.907% |             1,958,145 |         0.959% | -0.03% |     3.63% |   Yes    |
| `rome`             |        6,147,839 |    0.578% |             6,250,051 |         0.411% | +1.66% |     3.00% |   Yes    |

All twenty selected distributions have relative MAD at or below 1%, and all ten per-cell allowances pass. The geometric-mean candidate/base ratio is `0.9801203754139894`, a 1.99% improvement against the no-more-than-1% overall regression budget. The fixed-frequency inner and outer traps restored every pinned CPU to the original `performance` governor and 0.4–4.7 GHz limits, and both measurement worktrees remained clean.

The first formal behavior capture ran 440 fresh processes: eleven workloads, twenty with Rayon 1 and twenty with Rayon 4. Nine synthetic/Three workloads have one exact output and diagnostic value. Rome has one output digest, `6df16805cebdc8597ee43bd017785094`, and one exact forty-warning descriptor multiset; only scan completion order varies. The immutable `diagnostic-order` baseline has one exact descriptor multiset and one exact final four-error order; only the two independent cycle warnings exchange positions.

A Phase 3 development capture after execution-order extraction ran forty additional fresh `diagnostic-order` processes, twenty with Rayon 1 and twenty with Rayon 4. All forty emitted the exact A-then-B cycle order and exact four-error suffix. The unique digests are output `0e92238ee947f8b2482be635f01bb348`, pre-Generate `acba561fa5d80d60339b30c8a59d1e13`, final diagnostics `18adbc538dd6f79868238cc43f85bc5f`, and observation `8836305b971a187752d94ee86452be53`. The pre-Generate descriptor multiset and final diagnostic array otherwise match the immutable baseline. A one-process development smoke for every workload also matches the baseline manifests, module counts, output digests, and producer-aware diagnostic contract. This closes the known link-owned cycle-order ambiguity for the candidate; the complete Phase 3 digest corpus is rerun at the committed tree.

The same-base bundle-time control completed in the same fixed 2.6 GHz environment. The final comparison is `bundle-time-comparison.json`, SHA256 `17e8b3d23793deb1809a7bffafa4794ef2576062b1d797f3148c62c2fd36b2fb`; its selected-report manifest is `bundle-time-selection-final.tsv`, SHA256 `ef7827d87c0db79b6d3a94cff4ea2705f73b136ae7f6ca4f6f234037e722f198`.

| Workload           | Iterations | Base median (ns) | Base rMAD | Candidate median (ns) | Candidate rMAD | Change | Status          |
| ------------------ | ---------: | ---------------: | --------: | --------------------: | -------------: | -----: | --------------- |
| `overhead-64`      |         64 |        1,814,757 |    3.233% |                     — |              — |      — | `blocked-noise` |
| `wide-4096`        |          8 |       86,629,690 |    0.970% |            86,183,822 |         0.949% | -0.51% | accepted        |
| `deep-1024`        |         16 |       44,091,671 |    0.967% |            44,007,705 |         0.797% | -0.19% | accepted        |
| `scc-256x4`        |         64 |       25,210,556 |    0.819% |            25,246,386 |         0.631% | +0.14% | accepted        |
| `export-star-1024` |         64 |       34,798,658 |    0.541% |            34,573,314 |         0.568% | -0.65% | accepted        |
| `cjs-2048`         |         32 |       55,897,503 |    0.667% |            55,325,649 |         0.656% | -1.02% | accepted        |
| `json-2048`        |         16 |       74,219,214 |    0.570% |            68,436,810 |         0.691% | -7.79% | accepted        |
| `dynamic-1024`     |         16 |       33,695,989 |    0.832% |            33,794,397 |         0.733% | +0.29% | accepted        |
| `three-r108`       |         16 |       55,094,439 |    0.551% |            55,030,105 |         0.678% | -0.12% | accepted        |
| `rome`             |         16 |      121,430,974 |    0.666% |           121,105,737 |         0.458% | -0.27% | accepted        |

All nine stable bundle cells pass their per-cell allowances. Their candidate/base geometric-mean ratio is `0.9884590874`, a 1.15% improvement. `overhead-64` remains an evidence gap: its baseline relative MAD was 4.97%, 2.49%, and 3.23% at 1, 32, and 64 iterations per sample, so no candidate comparison is accepted and no regression or improvement is inferred.

The final trace comparison is `link-trace-structural-summary.json`, SHA256 `38a71989c43d14f814e7a8f1e28f85d2096acebb8b1b5598f773d39470e0730c`; its selection is `link-trace-selection.tsv`, SHA256 `8a72d249f81c2393587f6b84aa11a7f84dc343a6597789591c0b1d3feaff6ff0`. All 20 baseline/candidate reports and all 200 samples contain exactly one Link span, no detached pass, and valid interval identities. One-iteration duration relative MAD ranges from 0.903% to 9.466%; duration remains descriptive and is not evaluated against a regression budget.

The final RSS comparison is `rss-comparison-summary.json`, SHA256 `1650ed5a1247a251eca1b9b9615e804e8b12ed82fe84381a23673f355bacb598`; its selection is `rss-selection-final.tsv`, SHA256 `fa9677b2ef9d378f7d15317551a9c33f688664422091a28881f9be44e61b21c1`. The matrix contains 20 cells, 107 paired attempts, and 2,140 fresh processes. Three cells meet the 1% relative-MAD requirement on both sides and pass their median and p90 allowances:

| Mode       | Workload    | Base median (KiB) | Candidate median (KiB) | Median ratio | p90 ratio | Status   |
| ---------- | ----------- | ----------------: | ---------------------: | -----------: | --------: | -------- |
| `scan-rss` | `deep-1024` |            86,014 |                 85,920 |     0.998907 |  1.001997 | accepted |
| `link-rss` | `deep-1024` |            88,474 |                 88,602 |     1.001447 |  0.999324 | accepted |
| `link-rss` | `json-2048` |         1,235,534 |              1,233,610 |     0.998443 |  0.998315 | accepted |

The remaining seventeen RSS cells still have baseline or candidate relative MAD above 1% after six paired attempts and are recorded as `blocked-noise`; none is classified as `blocked-regression`. The complete performance evidence therefore contains no accepted regression, while claims for the noisy cells remain intentionally absent. An independent read-only audit recomputed provenance, hashes, medians, MAD, p90, allowances, retry counts, stop conditions, trace identities, and CPU restoration and found no actionable discrepancy.
