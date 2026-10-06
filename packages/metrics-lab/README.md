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
| `node harness.mjs parity [--pages /a,/b] [--mask <sel>]`          | **Rendered output vs the original build**, every pinned page, both builds rendered back to back in one browser session: first-screen + full-page pixels, page height, visible text, accessible names, new runtime errors, and the LCP element. Every full `scan` runs it; this runs it alone. The first scan/measure/parity of a target saves the original. `--pages` adds client-side routes (entry HTML files are found automatically); `--mask` hides a region the original renders differently on every load.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `node harness.mjs parity anchor [--replace]`                      | Show the saved original (commit, pages, masks, its LCP element). `--replace` makes the current build the original — a human decision, recorded in every later verdict.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `node harness.mjs parity approve <page> --reason "…"`             | A human accepts one exact, intended difference (keyed on the changed pixels and every judged difference — any further change reopens the page). Listed in every later verdict.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `node harness.mjs baseline`                                       | Pin the last measurement (and the build-side `.state.json`) as the fixed reference for every following `baselineDelta`. Refused while the build renders differently from the original, or was never compared with it (see [Rendered-output parity](#rendered-output-parity)).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `node harness.mjs defer <feature>` / `undefer <feature>`          | Rewrite that feature's marker block in `app/src/main.ts` between static import and post-paint `import()`. Rebuild afterwards.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `node harness.mjs status`                                         | Feature modes, entry size, last/baseline LCP.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `node harness.mjs serve [--port 4646]`                            | Serve `app/dist` for manual poking.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |

`measure` and `coverage` also take `--dist <dir>` (plus optional `--entry`,
`--features a,b`) to point at any other built app; candidates are then advisory
per-module (the agent finds the import seams itself).

## Rendered-output parity

An LCP number only means something if the page still renders what it rendered
before. Agent runs showed the failure this section exists for: a change that
broke a style, deferred a font that then never loaded, or let a placeholder become
the LCP element — the page still loaded, the functional check still passed, and the
"gain" was measured on a broken page. Sometimes the break IS the gain (a hidden hero
makes LCP measure a smaller, earlier element).

So every full `scan` (or `parity` alone) compares the current build with the
**original** and the verdict treats a difference as a gate: it OPENs, "keep this
change" turns into "NOT a keeper", and `scan --pin` / `baseline` refuse to pin.

**The original is a copy of the build, frozen.** The first scan / measure / parity of
a target copies the build directory into the lab state (`parity/anchor/`), with the
app's commit and a content hash. Each check serves it next to the current build from
one origin and renders both in the same browser session, so a Chrome update, a newly
installed font or machine load never shows up as a difference. Unlike the perf
baseline, the loop never re-pins it: compared to the step before, a broken step looks
clean and becomes the next step's reference; compared to the original, every step
answers the real question. `parity anchor --replace` exists for a human who decides
the reference must move, and every later verdict says it was replaced.

**Pages are pinned.** The original's entry HTML files are found automatically (files
that load bundled scripts or stylesheets — a self-contained `stats.html` is skipped);
client-side routes are added with `--pages`. Pages can be added at any time (the
original build is kept, so a new page is rendered from it on demand), never removed.

**Captures are made repeatable before anything is compared**, with the sequence
hosted visual-regression tools converged on (Chromatic, Percy, Cloudflare's Delta):
1280×900 at scale 1, no throttle, reduced motion, a CSS switch that zeroes
animations and transitions, `document.fonts.ready`, then animation-frame, DOM and
network quiet windows under one shared deadline (a page with an endless animation
loop stops being waited on after 2s), Web Animations and SMIL frozen right before
each screenshot, a pinned `Date` and a seeded `Math.random`, cleared storage per
capture, and a scroll through the page so sections that render on scroll exist in
both builds before the full-page shot.

**Noise is measured, not guessed.** Each page renders three times: original,
current, original again — the second original under a clock moved by 1d1h1m1s and
another random seed. Whatever differs between the two originals (a clock, random
content, a race) becomes that page's unstable mask; a difference there is reported
as "not judged", never as a break. Because both builds render on the same machine in
the same session, the remaining tolerance can be tiny (a channel move of more than
10/255 over at least 20 pixels) — a hosted service's 0.5%-of-the-screen default
would let a missing 24px icon through. A page whose original moves over more than
30% of the view is UNKNOWN: "nothing changed" would only cover the part that holds
still. A difference found once is confirmed by a second capture of the current
build before it is reported.

**What decides, and what explains.** Judged — any one opens the page:
first-screen and full-page pixels, page height, visible text (`innerText`, order-
insensitive), accessible names of controls/landmarks/headings/images, where the page
ended up, runtime errors the original never produced (uncaught exceptions,
`console.error`, failed requests, HTTP ≥ 400), and — for `/` — the LCP element from
the timed runs, compared with the original's: a different element, the same element
much smaller, or a placeholder (LCP fired on content the element later replaced: a
blurred image swapped for the real one, skeleton text swapped for copy, a skeleton
node removed). A same-size swap emits no new LCP entry, so the timestamp keeps
pointing at the placeholder; only the settled element shows it. Explanatory only —
they say why, never decide: computed styles per element, the font Chrome actually
drew each first-screen text element with (fallbacks included), images, loaded font
faces, and which elements sit under each changed region. A legitimate deferral
changes none of the judged facts but may change explanatory ones (a wrapper element,
a renamed image file).

**Masks and approvals are for humans, and visible.** `--mask <selector>` paints a
region opaque in every capture; one added after the original was saved is listed in
every verdict. `parity approve <page> --reason "…"` accepts one exact difference —
keyed on the changed pixels and every judged difference, so any further change
reopens the page — and is listed in every verdict with its reason.

Evidence per page lands in `parity/pages/<page>/`: `anchor.png`, `current.png`,
`diff.png` (changes red, unstable areas blue) and the `-full` variants. A check costs
three unthrottled loads per page (four when a difference is confirmed); on the
rolldown docs build that is ~2.5s per load.

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
   - **Rendered output must match the original**: the verdict's parity line is
     clear (`parity`, or any full scan). A faster build that renders differently is
     a revert too — and `baseline` refuses to pin it.
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
- `state/parity/` — `anchor/` (the original build's copy + `manifest.json`: commit,
  content hash, pinned pages and masks with the time each was added, the original's
  LCP element), `report.json` (per page: state, the judged reasons, notes, region
  explanations, approval signature), `approvals.json`, and `pages/<page>/*.png`.

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
- Parity sees what the build renders **served statically, without interaction**:
  states behind a click, a login or live API data are not compared (both builds
  render the same failed-fetch state, which is compared). Full pages are compared
  down to 10 viewport heights, styles explain the first 4000 elements, shadow-DOM
  content is compared by pixels only, and workers keep an unseeded `Math.random`.
  Cross-origin resources (third-party fonts, CDNs) are fetched live by both builds:
  a flaky one mostly shows up as instability between the two originals or fails the
  confirmation capture, but one that fails exactly while the current build is being
  captured (twice) reads as a difference.
