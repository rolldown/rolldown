# metrics-lab — browser-loading perf harness for agent loops

## Install / run

Zero runtime dependencies (raw CDP over Node ≥22's built-in WebSocket + a system
Chrome/Edge), so it works three ways:

- **In this repo**: `node packages/metrics-lab/harness.mjs …` from anywhere.
- **As a dependency**: `npm i -D <tarball or @rolldown/metrics-lab>` → `npx metrics-lab scan --app .`
  (the `bin` shim replaces any `node node_modules/…` path-typing). When installed,
  state lives in the consumer project at `.metrics-lab/` (add it to your
  `.gitignore`), never inside `node_modules`; override with `METRICS_LAB_STATE`.
- **Via a linked rolldown checkout**: if your project has
  `"rolldown": "link:<checkout>/packages/rolldown"`, the rolldown package's
  `rolldown-metrics` bin launches this harness from the sibling package —
  `npx rolldown-metrics scan --app .` with nothing else installed. State goes to
  your project's `.metrics-lab/`; on a registry-installed rolldown the launcher
  explains that the lab isn't bundled and points at `@rolldown/metrics-lab`.
- **Once published**: `npx @rolldown/metrics-lab scan --app .` with no install step.

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

| Command                                                           | One loop step                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `node harness.mjs scan --app <appDir>`                            | **The whole iteration in one browser session**: N timed runs + coverage + boot profile + the fused verdict. First scan of a target auto-pins the baseline; `--pin` re-pins after a kept change; `--quick` = 1 run + no profile, a fast mid-iteration probe on slow apps (indicative only — never pinned, and the verdict flags single-run measurements). The target is remembered — afterwards run everything bare, from any cwd. Per-target state dirs keep baselines/history from mixing across apps.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `node harness.mjs target [<appDir>] [--demo]`                     | Show / set / clear the remembered target.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `node harness.mjs gen [--force]`                                  | Generate the demo app (deterministic; `--force` resets defers).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `node harness.mjs build`                                          | Build `app/` → `app/dist/` + build metrics report.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `node harness.mjs measure [--runs 5] [--label X] [--no-throttle]` | N throttled runs (1 warmup discarded) → `state/runtime-metrics.json` with medians, guard, `delta`/`baselineDelta` — plus a **render-blocking CSS gate** flag (the last blocking stylesheet finished ≈ FCP: CSS, not JS, is the paint gate — via resource-timing `renderBlockingStatus`), a **render gap** flag (paint gated on post-load work; the gate is named: gating fetches, or per-type pre-paint resource weight — fonts/images) and a **pre-paint CPU** flag (long tasks before first paint).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `node harness.mjs coverage`                                       | One instrumented run → per-module bytes executed **before first paint** vs **by settle** → `state/coverage.json` + four lead sections: defer candidates, **cold bytes at paint** (the unified ranking `totalBytes − paintBytes`, coldest first — catches the mid-band a partially-initializing vendor SDK sits in, which neither the <2% nor the ≥50% bucket sees; framework runtimes annotated), large-modules-executed-at-paint (data evaluates on import — executed ≠ needed), and sibling variant groups (locales/themes). Covers the **whole initial load**: the entry chunk plus every same-origin chunk that executed before first paint (modules tagged with their chunk; chunks without sourcemaps are called out). Post-paint-executed chunks are split by fetch timing: **static pre-paint transfer** (fetched before paint via static tags/preloads — the paint paid for the download; verdict lead when ≥100KB) vs genuinely-deferred (fetched after paint). Entry auto-detected from `dist/index.html` — module scripts, or webpack-style plain script tags (main-looking/biggest bundle, query strings stripped); override with `--entry`. |
| `node harness.mjs profile`                                        | One profiled run → boot CPU by source module, navigation → first paint (`state/profile.json`). The follow-up to a pre-paint CPU flag.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `node harness.mjs graph`                                          | **Static split candidates ranked by retained size** — per module, the bytes its dominator subtree would remove from the initial load if its import edge were deferred, with `via` naming the single import chain to cut. Reads `module-graph.json` from a rolldown devtools-metrics build (vite ≥ 8: `build.rolldownOptions.devtools = { mode: "metrics" }`; the demo app emits it natively).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `node harness.mjs what-if <module> [--keep a,b]`                  | The exact modules + bytes one deferral frees (unique-reachable closure; equals the module's retained size). `--keep` marks sentry modules that stay eager. Instant candidate ranking; verify the winner's LCP effect with `scan`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `node harness.mjs verdict`                                        | Fuses all signals into an OPEN/clear/UNKNOWN lead checklist with staleness tracking. Refuses to say "done" while leads are open or signals are missing/stale; the all-clear states the tools' blind-spot boundary. While leads are OPEN it also instructs the operator/agent to copy the checklist into their summary and justify any early stop lead-by-lead — a re-pinned baseline records a gain, it does not close the checklist.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `node harness.mjs baseline`                                       | Pin the last measurement (and the build-side `.state.json`) as the fixed reference for every following `baselineDelta`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `node harness.mjs defer <feature>` / `undefer <feature>`          | Rewrite that feature's marker block in `app/src/main.ts` between static import and post-paint `import()`. Rebuild afterwards.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `node harness.mjs status`                                         | Feature modes, entry size, last/baseline LCP.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `node harness.mjs serve [--port 4646]`                            | Serve `app/dist` for manual poking.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |

`measure` and `coverage` also take `--dist <dir>` (plus optional `--entry`,
`--features a,b`) to point at any other built app; candidates are then advisory
per-module (the agent finds the import seams itself).

## Server startup mode (node / deno)

Same question — "what does the initial load pay for, and what can leave it?" — asked
of a process coming up instead of a page loading.

```bash
node harness.mjs node-scan --entry dist/server.js --ready port:3000
node harness.mjs node-scan            # target + readiness are remembered
node harness.mjs node-scan --pin      # make this the fixed reference
```

| Command                                    | What it does                                                                                                                                                                                                                                                                                                                        |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `node-scan`                                | Timed spawn→ready runs, then two instrumented runs: **what had to execute** to become ready (V8 precise coverage snapshotted at the readiness moment, attributed to source modules through the sourcemap) and **where the pre-ready CPU went** (sampling profile armed while the process is still paused at entry). Then the leads. |
| `node-measure`                             | Timing only — no instrumented runs.                                                                                                                                                                                                                                                                                                 |
| `graph` / `what-if` / `cut` / `graph-diff` | Work unchanged on a server build (`--dist <builtDir>` or `--report <dir>`): a top-level import and a render-blocking import are the same edge in the module graph.                                                                                                                                                                  |

**Readiness is part of the target.** A server has no equivalent of first paint, so the
target declares how it signals "up", and the spec is pinned with it — two scans that
measured different moments are not comparable.

| `--ready` spec                 | Ready when                                        |
| ------------------------------ | ------------------------------------------------- |
| `port:3000`                    | a TCP listener is accepting                       |
| `stdout:listening on \d+`      | a line matching this regex is printed             |
| `http://127.0.0.1:3000/health` | an HTTP probe returns 2xx                         |
| `exit`                         | the process exits 0 (CLIs, lambda-shaped entries) |

Other options: `--cache cold|warm|off`, `--args "..."`, `--exec <bin>` (deno, another
node), `--cwd <dir>`, `--runs`, `--timeout <seconds>`, `--label`, `--pin`.

Metrics are `runtime.startup_ms`, `runtime.boot_floor_ms` (what an empty script costs on
this machine) and `runtime.app_startup_ms` (the difference — the part a bundle change can
actually move), plus the same `delta` / `baselineDelta` shape the browser side writes.

### What this mode does NOT do

- **No throttle.** Server startup does no network I/O, so a throttle would invent a
  dimension the measurement does not have. There is therefore no net-scale calibration
  and no cross-scale comparability problem.
- **The reported timing never runs under the inspector.** `--inspect-brk` shifts startup
  by tens of ms; the instrumented runs are separate and used for attribution only.
- **Bun is not supported.** It runs JavaScriptCore, whose inspector is the WebKit
  protocol and whose profile/coverage formats are not V8's, so attribution cannot work
  there. Deno is V8 + CDP and rides the same driver via `--exec`.
- **Edge runtimes cannot be measured this way** — no local process, and cold start is
  dominated by platform isolate boot. The static half (`graph`, `what-if`, `cut`) still
  applies to an edge bundle; the measured half does not.

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
  (+ ratios), the sorted `candidates` list, and `coldAtPaint` (top modules by
  `totalBytes − paintBytes`, framework runtimes flagged).
- `state/rolldown-metrics/` — the build-side report (`output.max_initial_load_bytes`
  should drop with every accepted defer while `output.total_bytes` stays flat).

## The demo app

Client-rendered page (LCP = the hero `<h1>` painted by `main.ts`), ~381KB entry.
Each module demonstrates one case the loop must get right:

| Module               | ~KB | Behavior                                  | Expected verdict                       |
| -------------------- | --- | ----------------------------------------- | -------------------------------------- |
| `features/charts`    | 145 | runs post-paint (below fold)              | defer → big LCP win                    |
| `features/markdown`  | 107 | runs only on click                        | defer → big LCP win                    |
| `features/analytics` | 59  | runs post-paint                           | defer → win                            |
| `features/badges`    | 4   | runs post-paint, tiny                     | defer → within noise → revert          |
| `features/hero_data` | 25  | **executes before paint** (hero subtitle) | excluded by coverage, not by structure |
| `i18n`               | 39  | executes before paint (hero copy)         | excluded                               |

All weight lives inside function bodies so V8 coverage can separate "parsed" from
"ran before paint"; every feature reports readiness on `window.__ready` so the
guard catches a defer that broke behavior.

## Caveats

- Lab numbers are lab-only (fast-3G-ish throttle, 4× CPU, cold cache, localhost).
  The signal is the delta between builds under identical conditions.
- V8 coverage counts a module's top level as executed at evaluation, so real-world
  modules whose weight is top-level data look "used at paint" even if nothing reads
  them — a known blind spot; judge those by size + manual inspection. The same blind
  spot applies to `node-scan`'s cold-at-ready list.
- Server startup readiness discovered by polling (`port:` / `http://`) quantizes to the
  poll interval. When the addressable time is only a few intervals wide, `node-scan`
  says so rather than reporting millisecond deltas it cannot resolve — judge byte and
  module counts there, not milliseconds.
- `defer`/`undefer` is a marker-block codemod, i.e. demo-app sugar for what an agent
  does on a real codebase: rewriting the import site into a post-paint `import()`.
- Lab INP is meaningless (no real interaction); field metrics are Phase 3's
  beacon path, not this harness.
