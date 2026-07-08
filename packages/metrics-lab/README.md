# metrics-lab — browser-loading perf harness for agent loops

Prototype of the metrics plan's **Phase 3b (lab runner)** and **3c (code coverage)**:
the measurement and mutation primitives an agent needs to run a
"measure → find unused-at-paint code → lazy-load it → re-measure → accept or revert"
optimization loop against a real headless browser. The harness deliberately does
**not** run the loop itself — deciding what to try next and whether to keep it is the
agent's job; every command here is one loop step with machine-readable output.

No dependencies: raw CDP over Node's built-in `WebSocket` (Node >= 22) against a
system Chrome/Edge, and the repo's own rolldown (`packages/rolldown/dist`, so run
`just build-rolldown` first). Builds run with `devtools: { mode: 'metrics' }`, so
every build also refreshes the build-side report under `state/rolldown-metrics/`.

## Commands

| Command | One loop step |
|---|---|
| `node harness.mjs gen [--force]` | Generate the demo app (deterministic; `--force` resets defers). |
| `node harness.mjs build` | Build `app/` → `app/dist/` + build metrics report. |
| `node harness.mjs measure [--runs 5] [--label X] [--no-throttle]` | N throttled runs (1 warmup discarded) → `state/runtime-metrics.json` with medians, guard, `delta` (vs previous measure) and `baselineDelta` (vs pinned baseline). |
| `node harness.mjs coverage` | One instrumented run → per-module bytes executed **before first paint** vs **by settle** → `state/coverage.json` + defer candidates. Entry chunk auto-detected from `dist/index.html` (hashed Vite assets work); override with `--entry`. |
| `node harness.mjs baseline` | Pin the last measurement (and the build-side `.state.json`) as the fixed reference for every following `baselineDelta`. |
| `node harness.mjs defer <feature>` / `undefer <feature>` | Rewrite that feature's marker block in `app/src/main.ts` between static import and post-paint `import()`. Rebuild afterwards. |
| `node harness.mjs status` | Feature modes, entry size, last/baseline LCP. |
| `node harness.mjs serve [--port 4646]` | Serve `app/dist` for manual poking. |

`measure` and `coverage` also take `--dist <dir>` (plus optional `--entry`,
`--features a,b`) to point at any other built app; candidates are then advisory
per-module (the agent finds the import seams itself).

## The loop protocol (for an agent)

1. **Baseline**: `gen` → `build` → `measure --runs 5 --label baseline` → `baseline`.
2. **Find a candidate**: `coverage`. Candidates are modules ≥3KB with <2% of their
   bytes executed at first paint, largest first. Modules hot at paint (e.g. the
   demo's `i18n`, `hero_data`) are critical-path — never defer them, even though
   `hero_data` structurally could be.
3. **Mutate**: `defer <top candidate>` → `build`.
4. **Judge**: `measure --runs 5 --label "defer <name>"`, then read
   `state/runtime-metrics.json`:
   - **Guard must pass**: `guard.allFeaturesReady && guard.heroRendered &&
     guard.lcpObservedInAllRuns`, and `runtime.cls` must not grow by more than 0.02.
     A faster build that broke a feature is a revert, not a win.
   - **Improvement must beat noise**: `baselineDelta["runtime.lcp_ms"].delta` ≤
     −max(30ms, 2% of baseline). Judge by `baselineDelta`, not the chain `delta`.
5. **Decide**:
   - Accept → `baseline` (re-pin: this is the new reference).
   - Revert → `undefer <name>` → `build`, and don't retry that candidate.
6. **Repeat** from 2. **Converged** when no candidates remain, or 2–3 consecutive
   reverts, or the last accepted improvement is under ~2%.

Log the decision trail with `--label`; every measure also appends to
`state/history.jsonl`.

## Outputs

- `state/runtime-metrics.json` — flat metric ids (`runtime.lcp_ms`,
  `runtime.lcp_p75_ms`, `runtime.fcp_ms`, `runtime.ttfb_ms`, `runtime.load_ms`,
  `runtime.cls`, `runtime.transfer_bytes`, `runtime.js_request_count`), `guard`,
  per-run `samples`, `delta`, `baselineDelta` — same delta/baseline conventions as
  the build-side `metrics.json`.
- `state/coverage.json` — per-module `totalBytes` / `paintBytes` / `settleBytes`
  (+ ratios) and the sorted `candidates` list.
- `state/rolldown-metrics/` — the build-side report (`output.max_initial_load_bytes`
  should drop with every accepted defer while `output.total_bytes` stays flat).

## The demo app

Client-rendered page (LCP = the hero `<h1>` painted by `main.ts`), ~381KB entry.
Each module demonstrates one case the loop must get right:

| Module | ~KB | Behavior | Expected verdict |
|---|---|---|---|
| `features/charts` | 145 | runs post-paint (below fold) | defer → big LCP win |
| `features/markdown` | 107 | runs only on click | defer → big LCP win |
| `features/analytics` | 59 | runs post-paint | defer → win |
| `features/badges` | 4 | runs post-paint, tiny | defer → within noise → revert |
| `features/hero_data` | 25 | **executes before paint** (hero subtitle) | excluded by coverage, not by structure |
| `i18n` | 39 | executes before paint (hero copy) | excluded |

All weight lives inside function bodies so V8 coverage can separate "parsed" from
"ran before paint"; every feature reports readiness on `window.__ready` so the
guard catches a defer that broke behavior.

## Caveats

- Lab numbers are lab-only (fast-3G-ish throttle, 4× CPU, cold cache, localhost).
  The signal is the delta between builds under identical conditions.
- V8 coverage counts a module's top level as executed at evaluation, so real-world
  modules whose weight is top-level data look "used at paint" even if nothing reads
  them — a known blind spot; judge those by size + manual inspection.
- `defer`/`undefer` is a marker-block codemod, i.e. demo-app sugar for what an agent
  does on a real codebase: rewriting the import site into a post-paint `import()`.
- Lab INP is meaningless (no real interaction); field metrics are Phase 3's
  beacon path, not this harness.
