# Independent Vue project admission

This harness admits real Vue project entry graphs before any independent-project timing. It invokes Rolldown directly, never Vite, and never replaces a project graph with synthetic all-roots input. The frozen projects are Floating Vue, cabinet-fe/icon, PrimeVue as a workspace-resolution bridge, GitLab as the first large project, and Vben as the conditional large fallback. TDesign Vue Next and Directus are explicitly labelled protocol-amendment candidates and do not change the frozen matrix.

Every executable requires local Node.js 24.18.0, rejects active CI markers, and requires inherited `NODE_OPTIONS` to be unset. Correctness uses the lifecycle-corrected Rolldown profile only: source `b144106882fe244b19b738fc0acf3ffa07c7c9f3`, native binding `7b8863bb28aefd2e2eb7409f8be6dae57a252fe4a2688383007be7ea2f847bf7`, and distribution `1efffd0b63483e77cd2854fe716941000ae9548768691d7b5a64dceb011f3c45`. Pool capacities are pinned to Tokio 18, Rayon 12, and blocking 4. No run in this directory is CI timing evidence.

## Reproduction

Use the pinned Node binary and pass the exact lifecycle-corrected package distribution:

```sh
cd examples/par-plugin/cases/vue-projects

/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./prepare-projects.mjs

/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./test-verification.mjs

/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./test-performance-verification.mjs

/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs \
  ./correctness-matrix.json \
  /tmp/vue-project-correctness.json \
  /path/to/lifecycle-corrected-rolldown/packages/rolldown

/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-matrix.mjs \
  ./amendment-candidate-matrix.json \
  /tmp/vue-project-amendment-candidates.json \
  /path/to/lifecycle-corrected-rolldown/packages/rolldown
```

Preparation creates ignored, detached, clean checkouts under `tmp/bench/vue-projects`, verifies commits, licenses, physical SFC manifests, and entries, and installs only the dependency subsets required by the Vben and Directus graphs with frozen lockfiles and lifecycle scripts disabled. It invokes the exact package manager through the Node 24.18.0 Corepack executable: Vben uses `corepack pnpm@11.7.0 install --filter @vben/web-antd... --ignore-scripts --frozen-lockfile`; Directus uses `corepack pnpm@10.27.0 install --filter @directus/app... --ignore-scripts --frozen-lockfile`. Preparation then verifies the installed pnpm lock hash, `.modules.yaml` package-manager/layout fields, and hashes and versions of critical packages. Directus still declares Node 22; the report retains its deliberate Node 24.18.0 mismatch.

Every matrix case declares its exact expected child exit code, execution status, and admission status. The runner rejects a missing expectation or any mismatch before it can be summarized. `correctness-goldens.json` also fixes the already established transform, graph, output, dependency, compiler-contract, glob-expansion, and workspace-resolution evidence. A transform-result record hashes both the byte length and SHA-256 of the transformed code after replacing the exact absolute prepared-project root with `<project-root>`; this removes only checkout relocation from compiler-generated style-block imports while retaining changes to code, query strings, and paths below the project root. `correctness-matrix.json` runs two fresh ordinary processes before worker smoke for executable graphs; GitLab has one expected `not-run` semantic rejection and no worker. Floating Vue, cabinet-fe/icon, PrimeVue, and the mechanically complete but scale-rejected Vben graph then run worker one and worker four. A worker variant must match the normalized ordinary evidence exactly. `amendment-candidate-matrix.json` applies the same gates to viable candidates without adding them to the frozen protocol.

Each successful matrix invocation writes the complete raw JSON and a sibling `.summary.json`. The summary retains the exact raw-file hash plus a canonical hash over normalized correctness evidence. A summary is durable only when both the research harness and lifecycle runtime worktrees are clean; provisional runs from dirty source are deliberately marked `durableEligible: false` and must not be copied into the research verdict. Before writing either artifact, the runner re-verifies the prepared project snapshots, runtime distribution, harness tree, Node/toolchain packages, matrix, and golden. `test-verification.mjs` contains negative tests proving that unexpected exit codes, execution/admission statuses, missing expectations, inherited `NODE_OPTIONS`, package-entry drift, and golden drift are rejected.

`wall-screen-matrix.json` is the superseded planning placeholder. `performance-wall-screen-matrix.json` is the Amendment 4 formal screen for Floating Vue, cabinet-fe/icon, and Directus. `run-performance.mjs` requires a clean worktree, the exact lifecycle runtime, current durable correctness raw/summary evidence, no inherited `NODE_OPTIONS`, a qualifying host before and after every child, and zero pageout/swapout delta. Direct `run-case.mjs` rejects `collectPerformance: true` unless the formal orchestrator supplies the frozen protocol marker.

Validate the formal orchestration without running a host gate or build:

```sh
/Users/yunfeihe/.local/share/mise/installs/node/24.18.0/bin/node ./run-performance.mjs \
  ./performance-wall-screen-matrix.json \
  /tmp/independent-vue-screen.json \
  /path/to/lifecycle-corrected-rolldown/packages/rolldown \
  --validate-only
```

After a clean committed correctness rerun, store the small/medium and Directus raw/summary pairs in a commit of the separate `github.com/hyf0/rolldown-parallel-js-plugin` repository. The manifest contract is schema 2 and lives at `research/artifacts/correctness/sha256/<contentSha256>/manifest.json`; raw files live at `raw/<rawSha256>.json`, summary files at `summary/<summarySha256>.json`, and `contentSha256` is SHA-256 over the byte-sorted `rawSha256 + NUL + summarySha256 + LF` pairs. The manifest declares `artifactStore.kind: "git-head-content-addressed"`, the exact repository, root, content hash, and artifact hashes. Commit all files before use: the validator rejects untracked files, working-tree bytes that differ from `HEAD`, obsolete schema-1 manifests, noncanonical paths, wrong hashes, and the wrong remote. It also recomputes the complete normalized correctness hash for every admitted ordinary and worker run, requires exact per-project equality, and freezes that project reference into formal evidence. Every screen and confirmation child must equal the committed reference, not merely the other children in its own matrix. The same manifest remains valid from a fresh clone because reports retain repository-relative paths rather than checkout paths. Pass the committed manifest with `--correctness-evidence`. Formal raw and compact outputs must be outside the Rolldown worktree. Generate the rotated confirmation matrix with `create-performance-confirm-matrix.mjs`; it selects the fastest screened resource-envelope-safe count, its adjacent lower and higher counts, and fixed worker-four and worker-eight policy candidates, deduplicating overlaps. It then uses fifteen blocks when the canonical ordinary and selected screen times are both below two seconds and ten otherwise. Confirmation execution additionally requires `--screen-evidence SCREEN_RAW SCREEN_SUMMARY`, verifies the raw hash frozen into the generated matrix, regenerates the compact screen summary, and rechecks the same committed correctness reference before accepting it.

The canonical child wall measurement for screening, the two-second repeat rule, paired statistics, and worker selection is `/usr/bin/time -l` real time, recorded as `timeRealMs`. A parent `process.hrtime.bigint()` span is stored exactly as decimal `parentWallNs`; `childWallMs` is its exact derived value and `parentWallOverheadMs` is exactly `childWallMs - timeRealMs`. The parent span is only a spawn-boundary sanity measurement: it must contain the printed canonical wall within `/usr/bin/time`'s recorded precision, that precision may be no coarser than 10 ms, and positive parent overhead may not exceed 250 ms. The compact confirmation summary exposes repeated fixed-four and fixed-eight results and a `policyEvidence` block per project. Its `variants` object provides stable JSON Pointer targets for ordinary and every repeated worker candidate: `wallMedianMs` is the canonical real-time median, `cpuMedianMs` is the median child user-plus-system time, `peakRssMedianBytes` is the median maximum resident set size, worker `resourceEligible` applies the frozen benefit and resource limits while ordinary is the admitted reference, and `pairedWallRatioBootstrap95Upper` is the upper interval endpoint for the paired worker/ordinary wall ratio. `selectedOracleWorkerCount` records the repeated-time oracle selected by the frozen tie rule.

The compact performance summary reports per-project mechanical and resource results. It explicitly does not infer a crossover by ordering the three unrelated project families; the nested schema-2 controlled Vue corpus remains the crossover source. Product crossover remains unavailable while transform source-map correctness and diagnostic/failure parity are untested.

## Graph support boundary

Repository-local relative edges, declared aliases, generated entries, import-meta globs, and checked-out workspace packages stay inside the graph. Unresolved local edges fail. Bare third-party dependencies remain external. PrimeVue self-imports resolve to `packages/primevue/src`; Vben package exports resolve to workspace source because its production dist is not built; Directus package exports whose pinned dist is not built resolve to the corresponding checked-out source. Directus catalog packages such as `@directus/license` and `@directus/vue-split-panel` remain external.

The narrow project-support transform expands only literal `import.meta.glob` strings or arrays used by the pinned projects, including generic type arguments, eager/default selection, and raw queries. Each expansion retains its importer, source offset, expression hash, patterns, options, and sorted file list in a frozen manifest. Workspace package discovery and every checked-out workspace resolution edge have separate sorted manifests. It does not supply a Vite runtime. Local raw, YAML, asset URL, Sass/CSS, and Vue style child requests use deterministic loader replacements so they cannot turn into unresolved local edges. These replacements and the transform-only adapter's omitted style/custom-block semantics are recorded capability boundaries; this lane is graph and transform correctness evidence, not product crossover evidence.

The adapter explicitly supplies its own `vue/compiler-sfc` 3.5.39 compiler to every project, so a project's installed Vue version cannot silently choose a different compiler. It deliberately loads the Node wrapper rather than requiring `@vue/compiler-sfc` directly because the wrapper registers TypeScript filesystem support needed by imported SFC types. Provenance freezes the Node binary, root and installed pnpm locks, `.modules.yaml`, package/source manifests, the actual `unplugin-vue/rolldown` entry (`cf2382af…`) plus its complete package payload (`92cc4139…`), the Vue wrapper entry (`a2226c3e…`) plus its complete package payload (`e1ba4d43…`), and the underlying `@vue/compiler-sfc` entry (`36048750…`) plus its complete package payload (`47016522…`). Package-local `node_modules` directories are excluded from payload hashes because pnpm-generated command shims embed checkout paths; dependency resolution is instead frozen by the installed lock and modules metadata. The audited adapter source manifest is `44e5c652…`. Output source-map hashes establish ordinary/worker artifact parity only. Source-map position correctness is untested because the transform adapter disables its own map, and plugin diagnostic parity is untested because this matrix has no paired invalid-SFC or worker-failure case.

See [admission findings](./admission-findings.md) for the retained outcomes and exact hashes.
