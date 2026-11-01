# perf-kit — performance harness for agent loops

## Install / run

Zero runtime dependencies (raw CDP over Node ≥22's built-in WebSocket + a system
Chrome/Edge), so it works three ways:

- **In this repo**: `node packages/perf-kit/harness.mjs …` from anywhere.
- **As a dependency**: `npm i -D <tarball or @rolldown/perf-kit>` → `npx perf-kit scan --app .`
  (the `bin` shim replaces any `node node_modules/…` path-typing). When installed,
  state lives in the consumer project at `.perf-kit/` (add it to your
  `.gitignore`), never inside `node_modules`; override with `ROLLDOWN_PERF_KIT_STATE`.
  The hints print the spelling that applies. No rolldown checkout sits next to an
  installed copy, so `rolldown-install` needs `--from <a built packages/rolldown>`.
- **Once published**: `npx @rolldown/perf-kit scan --app .` with no install step.

Prompts for an agent, one per target (browser page load, node or deno server startup,
Cloudflare Worker startup): `PROMPTS.md`.

Prototype of the metrics plan's **Phase 3b (lab runner)** and **3c (code coverage)**:
the measurement and mutation primitives an agent needs to run a
"measure → find unused-at-paint code → lazy-load it → re-measure → accept or revert"
optimization loop against a real headless browser. The harness deliberately does
**not** run the loop itself — deciding what to try next and whether to keep it is the
agent's job; every command here is one loop step with machine-readable output.

No dependencies: raw CDP over Node's built-in `WebSocket` (Node >= 22) against a
system Chrome/Edge, and the repo's own rolldown (`packages/rolldown/dist`, so run
`just build-rolldown` first). Builds run with `devtools: { mode: 'report' }`, so
every build also refreshes the build-side report under
`app/node_modules/.rolldown/report/dist/`.

## Commands

### Chunk discovery

`scan` keeps the full resource detail list in `runtime-metrics.json`. This includes
small preload facades. Each request records its initiator type and source location.
`requestPriority` records Chrome's initial priority, its last observed priority,
and changes with milliseconds since the request began. Use this history when
comparing preload priorities. The last priority is the value at collection, not
necessarily at paint. Repeated URLs and redirect hops have ambiguous matches;
the report omits their initiator and priority evidence. Missing evidence is not
proof of low priority. These fields use the
[CDP Network protocol](https://github.com/ChromeDevTools/devtools-protocol/blob/master/json/browser_protocol.json).
The report does not collect request headers or bodies. A browser timing buffer
overflow keeps the discovery check UNKNOWN.

The check reads `chunk-graph.json` from rolldown metrics. It can also use a Vite
manifest at `dist/.vite/manifest.json`. It follows static imports only. The report
stores separate results in `chunkDiscovery.fcp` and `chunkDiscovery.lcp`.
Each result uses that paint's time as the download deadline. A candidate starts
more than one configured RTT after its ancestor finishes. A download that finishes
after FCP can appear only in the LCP result. Missing LCP does not discard FCP evidence.

Each group includes filenames, bytes, and the number of affected runs. Its example
keeps one run's timings, import chains, and available source modules and initiators.
Groups and paint windows overlap. Do not add their bytes or delays.

The report marks each candidate's `paintDependency` as `unverified`. Check the
source to find whether the chunk renders the first visible content or the LCP
element. If paint needs it, test an earlier preload or a different chunk boundary.
If paint does not need it, test deferring its load and work. A discovery delay is
not a predicted paint gain. Earlier downloads can compete with other required assets.

The render-gap advice shows concurrent script, fetch/XHR, font, and image evidence.
Check which requests rendering awaits before changing load order. Keep the LCP
image request early. Compare these candidates with the CPU profile.

`paintComparison` checks both FCP and LCP against the pinned baseline. A change
must improve at least one metric beyond noise. Neither may regress beyond noise.
The threshold for each metric is max(30ms, 2% of its baseline). Missing metrics
leave the comparison unknown. Quick and early-stopped scans are probes. Use
`scan --no-early-stop` for a keep/revert decision. Check the app's features before
keeping a change. Check rendered output with `scan --parity` when the work is complete.

The render-gap report also counts scripts that start after `load` and finish
before FCP or LCP. The load event does not prove that downloads ended. Commands
that resolve a page target print its path, including remembered targets.

Static module analysis starts at build entries. It does not price a dynamic import
that startup immediately executes. Check coverage and CPU evidence before treating
small static entry totals as small startup totals. Manually grouped chunks can
also load modules outside an observed facade's static closure.

### CPU source attribution

`profile` maps sampled work through every same-origin chunk's sourcemap. This
includes static siblings and dynamic imports. `profile.json` records each source,
its chunk filenames, and sampled self-time. It separates source work, engine work,
and unattributed work. Missing or invalid maps, unmapped positions, inline scripts,
and external scripts remain visible. They do not count as engine work. A frame
without a URL (a builtin such as `JSON.parse` or a DOM call) counts for its nearest
caller with a URL, or as engine work when it has none. Top-level code of a bundled
chunk runs many modules in one frame: its time splits by the sampled lines, and a
line that holds more than one source stays `top-level` (unattributed). Minified
chunks keep most top-level time unattributed; profile an unminified build to split it.

The verdict keeps source modules with at least 15ms of sampled work OPEN for
inspection. A filename without a directory can also identify a source module.
Unattributed rows of at least 15ms keep the check UNKNOWN; a smaller row cannot
hide a source module that reaches the threshold. Old profiles that map only the entry chunk
must be collected again. A clear CPU signal means no source reaches the threshold;
it does not prove that all startup work is required.

The profile stops when the harness observes FCP. The polling tail can include
post-paint work. Sampled time is a lead, not a predicted paint gain. Check whether
the first render needs each source module. Test deferring optional work, then
compare both FCP and LCP in timed runs. The profile does not attribute CPU between
FCP and LCP when those paints differ.

### Command list

| Command                                                                          | One loop step                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `node harness.mjs scan --app <appDir>`                                           | **The whole iteration in one browser session**: N timed runs + coverage + boot profile + the fused verdict. First scan of a target auto-pins the baseline; `--pin` re-pins after a kept change; `--quick` = 1 run + no profile, a fast mid-iteration probe on slow apps (indicative only — never pinned, and the verdict flags single-run measurements). The target is remembered — afterwards run everything bare, from any cwd. Per-target state dirs keep baselines/history from mixing across apps.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `node harness.mjs scan --bandwidth <Mbit/s> [--rtt <ms>] [--cpu <rate>]`         | Pin a network in real units instead of a net-scale step: the network a deployed environment measured (a record's model), e.g. `--bandwidth 26 --rtt 120 --cpu 1` (1 = this machine's CPU). No calibration; RTT and CPU default to 150 ms and 4x. Pinned and compared like `--net-scale`: a change makes the pinned baseline incomparable (re-pin). Same flags on `measure` and `target`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `node harness.mjs target [<appDir>] [--demo]`                                    | Show / set / clear the remembered target.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `node harness.mjs gen [--force]`                                                 | Generate the demo app (deterministic; `--force` resets defers).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `node harness.mjs build`                                                         | Build `app/` → `app/dist/` + build metrics report.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `node harness.mjs measure [--runs 5] [--label X] [--no-throttle]`                | N throttled runs (1 warmup discarded) → `state/runtime-metrics.json` with medians, guard, `delta`/`baselineDelta` — plus a **render-blocking CSS gate** flag (the last blocking stylesheet finished ≈ FCP: CSS, not JS, is the paint gate — via resource-timing `renderBlockingStatus`), a **render gap** flag (paint gated on post-load work; the gate is named: gating fetches, or per-type pre-paint resource weight — fonts/images) and a **pre-paint CPU** flag (long tasks before first paint).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `node harness.mjs coverage`                                                      | One instrumented run → per-module bytes executed **before first paint** vs **by settle** → `state/coverage.json` + four lead sections: defer candidates, **cold bytes at paint** (the unified ranking `totalBytes − paintBytes`, coldest first — catches the mid-band a partially-initializing vendor SDK sits in, which neither the <2% nor the ≥50% bucket sees; framework runtimes annotated), large-modules-executed-at-paint (data evaluates on import — executed ≠ needed), and sibling variant groups (locales/themes). Covers the **whole initial load**: the entry chunk plus every same-origin chunk that executed before first paint (modules tagged with their chunk; chunks without sourcemaps are called out). Post-paint-executed chunks are split by fetch timing: **static pre-paint transfer** (fetched before paint via static tags/preloads — the paint paid for the download; verdict lead when ≥100KB) vs genuinely-deferred (fetched after paint). Entry auto-detected from `dist/index.html` — module scripts, or webpack-style plain script tags (main-looking/biggest bundle, query strings stripped); override with `--entry`. |
| `node harness.mjs profile`                                                       | One profiled run → boot CPU by source module, navigation → first paint (`state/profile.json`). The follow-up to a pre-paint CPU flag.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `node harness.mjs graph`                                                         | **Static split candidates ranked by retained size** — per module, the bytes its dominator subtree would remove from the initial load if its import edge were deferred, with `via` naming the single import chain to cut. Reads `module-graph.json` from a rolldown devtools report build (vite ≥ 8: `build.rolldownOptions.devtools = { mode: "report" }`; the demo app emits it natively).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `node harness.mjs what-if <module> [--keep a,b] [--root a,b]`                    | The exact modules + bytes one deferral frees (unique-reachable closure; equals the module's retained size). `--keep` marks sentry modules that stay eager. `--root` (also on `cut`) adds modules that load on every start without a static import from an entry - a dynamic import the app always runs: they join the entries, so the query prices that startup load. Instant candidate ranking; verify the winner's LCP effect with `scan`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `node harness.mjs verdict`                                                       | Fuses all signals into an OPEN/clear/UNKNOWN lead checklist with staleness tracking. Refuses to say "done" while leads are open or signals are missing/stale; the all-clear states the tools' blind-spot boundary. While leads are OPEN it also instructs the operator/agent to copy the checklist into their summary and justify any early stop lead-by-lead — a re-pinned baseline records a gain, it does not close the checklist.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `node harness.mjs parity [--only /a,/b] [--label x] [--exit-code]`               | **Rendered output vs the original build**, both rendered back to back in one browser session: first-screen + full-page pixels, page height, visible text, accessible names, new runtime errors, and the LCP element. Compared once, when the optimizing is done (`scan --parity` = final measurement + this). A difference starts recover mode. `--only` re-checks some pages, `--exit-code` exits 1 while a page differs (`git bisect run`), `--pages` / `--mask` add routes / masks (also on `scan`).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `node harness.mjs parity recover` / `strategy each-tweak\|on-demand` / `history` | Recover mode: what to do and how to bisect; re-check on every scan or when you decide; every check with its snapshot images.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `node harness.mjs parity anchor [--replace]`                                     | Show the saved original (commit, pages, masks, its LCP element). `--replace` makes the current build the original — a human decision, recorded in every later verdict.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `node harness.mjs parity approve <page> --reason "…"`                            | A human accepts one exact, intended difference (keyed on the changed pixels and every judged difference — any further change reopens the page). Listed in every later verdict.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `node harness.mjs baseline`                                                      | Pin the last measurement (and the build-side `.state.json`) as the fixed reference for every following `baselineDelta`. Refused for a build known to render differently from the original (see [Rendered-output parity](#rendered-output-parity)).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `node harness.mjs defer <feature>` / `undefer <feature>`                         | Rewrite that feature's marker block in `app/src/main.ts` between static import and post-paint `import()`. Rebuild afterwards.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `node harness.mjs status`                                                        | Feature modes, entry size, last/baseline LCP.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `node harness.mjs serve [--port 4646]`                                           | Serve `app/dist` for manual poking.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |

`measure` and `coverage` also take `--dist <dir>` (plus optional `--entry`,
`--features a,b`) to point at any other built app; candidates are then advisory
per-module (the agent finds the import seams itself).

`graph`, `what-if`, `cut`, `why` and `graph-diff` read the report of the target dist
(`node_modules/.rolldown/report/<output dir>`), and stop with an error when none of
the report's entry chunks exists in that dist: a rebuild without the report leaves
the old report behind, and builds that share an outDir share a report. Rebuild with
the report on, or pass `--report <dir>`, which is never checked.

## Rendered-output parity

An LCP number only means something if the page still renders what it rendered
before. Agent runs showed the failure this section exists for: a change that
broke a style, deferred a font that then never loaded, or let a placeholder become
the LCP element — the page still loaded, the functional check still passed, and the
"gain" was measured on a broken page. Sometimes the break IS the gain (a hidden hero
makes LCP measure a smaller, earlier element).

So the finished work is compared with the **original** — **once, at the end**, not
per change: an agent that broke a page can usually find out why from a good
description of the difference, and most changes don't break anything, so a check on
every iteration would mostly buy time back for nothing.

- **While optimizing** nothing is captured. Each measurement only says whether the
  LCP element is still the original's — free, the timed runs already observe it —
  as information. Pinning does not wait on a check.
- **When done**, `scan --parity` takes the final measurement and compares. The
  verdict holds the comparison as `[PENDING]` until then, so "done" needs it.
- **A difference starts recover mode.** The build's LCP numbers don't count until it
  matches again (the keep line says "NOT a keeper", pinning refuses a build known to
  differ). The agent fixes or reverts and picks how to re-check: `parity strategy
  each-tweak` (every scan re-checks and records a snapshot) or `on-demand` (it runs
  `parity` itself, `--only /page` for the broken pages). `parity --exit-code` exits 1
  while a page differs, so `git bisect run` can find the kept change that broke it,
  and `parity history` lists every check with its snapshot images. A check that
  covers every page without a difference ends recover mode.

**The original is a copy of the build, frozen.** The first scan / measure / parity of
a target copies the build directory into perf-kit's state (`parity/anchor/`), with the
app's commit and a content hash, and the first full scan records its LCP element. A
check serves it next to the current build from one origin and renders both in the
same browser session, so a Chrome update, a newly installed font or machine load
never shows up as a difference. Unlike the perf baseline, the loop never re-pins it:
the finished work is compared with what the app rendered before any change.
`parity anchor --replace` exists for a human who decides the reference must move,
and every later verdict says it was replaced.

**Pages are pinned.** The original's entry HTML files are found automatically (files
that load bundled scripts or stylesheets — a self-contained `stats.html` is skipped);
client-side routes are added with `--pages`. Pages can be added at any time (the
original build is kept, so a new page is rendered from it on demand), never removed.

**Captures are made repeatable before anything is compared**, with the sequence
hosted visual-regression tools converged on (Chromatic and Percy):
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
`diff.png` (changes red, unstable areas blue) and the `-full` variants, copied per
check into `parity/snapshots/<n>/`. A check costs three unthrottled loads per page
(four when a difference is confirmed); on the rolldown docs build that is ~2.5s per
load. A build with the original's exact files costs nothing: same files, same
rendering.

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
- **Other edge runtimes cannot be measured this way** — no local process. Cloudflare
  Workers have one (workerd); see the next section. The static half (`graph`, `what-if`,
  `cut`) still applies to any edge bundle; the measured half does not.

## Worker startup mode (Cloudflare Workers)

The same question asked of a V8 isolate coming up under workerd: what does the Worker's
global scope pay for, and what can leave it? Cloudflare's startup limit counts that
scope, and `wrangler check startup` profiles it; this mode adds the loop around it:
a floor-corrected number per run, what executed, and what each deferral would free.

```bash
node harness.mjs worker-config --app <appDir>          # writes perf-kit.vite.config.ts, prints the build commands
(cd <appDir> && npx vite build --config perf-kit.vite.config.ts)
node harness.mjs worker-scan --app <appDir> --request / --request /api/health   # routes the app answers (2xx)
node harness.mjs worker-scan --request 'ws:/agents/chat/a|{"type":"ping"}'     # a WebSocket route: upgrade, send, time to the last reply
node harness.mjs worker-scan                           # target, requests and --tools are remembered
node harness.mjs worker-scan --pin                     # make this the fixed reference
```

| Command                            | What it does                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rolldown-install --app <dir>`     | Copies a built rolldown package (with its native binding) over the copy the app's vite resolves; the default source is the rolldown package next to perf-kit, `--from <packages/rolldown>` names another. The npm release has no report mode; `worker-config` and the config shim refuse to build on it, because such a build writes the full event log (hundreds of MB) instead of the report. `<package manager> install --force` restores the release. |
| `worker-config`                    | Writes a Vite config that wraps the app's own: the rolldown metrics report on **every** environment (one directory per output dir), sourcemaps on the worker, a worker-only rebuild (`ROLLDOWN_PERF_KIT_WORKER_ONLY=1`) and the attribution build (`ROLLDOWN_PERF_KIT_ATTRIB=1`, `preserveModules` into `dist-attrib/`).                                                                                                                                  |
| `worker-scan`                      | N cold runs, a fresh workerd each: the main module's import timed from the host minus an empty-import floor = **app startup**. Then the first requests on the real entry, a CPU profile of the import attributed through every chunk's sourcemap, precise coverage of the import taken from inside the isolate, and the leads: cold and idle bytes per module and per package, runtime modules loaded, class exports.                                     |
| `worker-measure`                   | Timing and first requests only.                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `worker-attrib`                    | CPU by **evaluating module** from the attribution build, each row with its import chain: the only view that charges a library's top-level work (zod schemas built at import, a YAML parse) to the module whose evaluation runs it.                                                                                                                                                                                                                        |
| `why <module>`                     | Direct static importers and one chain from the entry.                                                                                                                                                                                                                                                                                                                                                                                                     |
| `cut <module> --root a,b`          | The fewest imports to make dynamic, with the Durable Object / Workflow class modules as roots: their closure stays eager, so the cut lands below them instead of at the entry.                                                                                                                                                                                                                                                                            |
| `graph` / `what-if` / `graph-diff` | Unchanged on the Worker's report (`node_modules/.rolldown/report/dist/<worker>`).                                                                                                                                                                                                                                                                                                                                                                         |

**Why the startup sits inside a request.** workerd evaluates a Worker's main module when
it loads the worker, before any request or inspector can observe it. perf-kit therefore
runs a generated wrapper entry whose fetch handler does `await import('./index.js')`:
that import compiles and evaluates the main module and its whole static closure. The
first requests and parity use a plain instance with the real entry instead, because
Durable Object classes live in the main module. `wrangler check startup` uses the same
wrapper trick.

**How coverage differs from the other modes.** Browser and node mode arm coverage from
outside, over the inspector, before any code runs. workerd answers `Profiler.enable`
itself and never forwards it to V8, so `Profiler.startPreciseCoverage` fails there. The
coverage run adds three compatibility flags for that run only, `nodejs_compat`,
`enable_nodejs_inspector_local_dev` and `enable_nodejs_inspector_module` (the module is off
before its default-on date), under which a wrapper opens a `node:inspector`
session inside the isolate, starts coverage, imports the main module and returns the
result in the response. It needs Wrangler 4.108.0 or later (workerd 1.20260706.1); for an
app that pins an older one, `--tools <dir>` borrows a newer wrangler for that run only.

**Requirements the scan checks for and names when unmet**

| Need                  | Requirement                                                                                                                                                                                          |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| the target            | a `@cloudflare/vite-plugin` build: `dist/<worker>/wrangler.json` next to `index.js`                                                                                                                  |
| the runtime           | `wrangler` as a dev dependency of the app (brings miniflare and workerd)                                                                                                                             |
| the report            | the `worker-config` build, or `devtools: { mode: 'report' }` on every environment by hand, on a rolldown with the report mode: the npm release has none yet, so `rolldown-install --app <dir>` first |
| sourcemaps            | `build.sourcemap: true` on the worker environment (the config shim sets it)                                                                                                                          |
| precise coverage      | Wrangler >= 4.108.0, or `--tools <dir>`                                                                                                                                                              |
| first-request numbers | the app's variables (a `.dev.vars` next to the config); without them both real apps answered 500                                                                                                     |
| the number for a PR   | `wrangler check startup` (Wrangler >= 4.116.0), before and after; its dry-run build runs Docker for `containers`                                                                                     |

Metrics are `worker.startup_ms`, `worker.boot_floor_ms`, `worker.app_startup_ms` (the part a
bundle change can move), `worker.instance_boot_ms` (reported, never counted) and
`worker.request_ms:<path>` per pinned request, with the same `delta` / `baselineDelta`
shape as the other modes. A request's delta against the pinned baseline is the cost a
deferral **moved** into that route; the summary prints it next to the startup gain.

### What this mode does NOT do

- **No throttle, no production number.** Local CPU differs from Cloudflare's; the ratio
  before and after on one machine is the claim, and Cloudflare's own number comes from a
  deploy or version upload.
- **No isolate boot.** Process start, config load and wasm compilation are outside the
  window and outside Cloudflare's startup limit too.
- **No timing from inside the Worker.** Timers in workerd advance only on I/O.
- **No response parity yet.** The first requests report status and size; a replayed
  request set against the frozen original is not built.
- **The attribution build works around a rolldown limit (rolldown#11348).** With `preserveModules` every
  CommonJS file ends with an eager `export default require_x()`, and a require cycle reaches
  a wrapper before its own file ran (`require_x is not a function`). The config shim drops
  that line in the attribution build only; its importers use the named wrapper. If an import
  still fails, `worker-attrib` says so and marks its rows partial.

## Field mode (a deployed site)

The lab answers "what does this build pay for" on a local copy with a fixed throttle.
Field mode asks the same question of the **deployed** site, in your own logged-in
Chrome: real data, real network, real server time. Use it when the lab and production
disagree, or to check that a gain survives a fast network.

```bash
# 1. Chrome: open a logged-in tab on the site, turn on chrome://inspect/#remote-debugging
# 2. Describe how the site serves each build:
node harness.mjs field --config field.json            # 6 alternating pairs + 1 diagnostic run each
node harness.mjs field --config field.json --runs 10 --cut lcp --out /tmp/my-field
```

```json
{
  "url": "https://app.example.com/",
  "variants": {
    "base": {
      "cookies": [{ "name": "version", "value": "stable" }],
      "build": "/builds/stable/dist"
    },
    "candidate": { "query": { "preview": "pr-123" } }
  },
  "block": ["analytics.example.com"],
  "ignoreLcp": "#splash",
  "gate": "/api/items"
}
```

| Part           | What it does                                                                                                                                                                                                                                                                                                   |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Timing runs    | Pairs of loads, one per variant, in alternating order, so a slow minute of the network hits both sides. A hidden page (covered window) or a missing LCP makes a run invalid; it is retried, never averaged in.                                                                                                 |
| Critical path  | The request LCP waited for (`gate`: a substring of a fetch/XHR URL, matched after redaction; unset: the last fetch/XHR that ended before LCP), split into: document end, client time before the gate request, gate TTFB, download, client time after it. Only the client parts can move.                       |
| Script chain   | What the gate request waited for: of all scripts on its initiator stack, the one that finished loading last, then that script's own initiator, back to the document. The shape comes from the diagnostic run (async stacks name the caller behind a fetch wrapper); the times come from the median timing run. |
| Diagnostic run | V8 precise coverage at the cut (`--cut gate`: when the gate request is sent; `lcp`; `fcp`) and at settle, plus a CPU profile to the cut. Slower by design and never used for timing. A run that does not reach its cut in `--timeout` is invalid.                                                              |
| Build match    | Each served script is matched to a local build file by SHA-256 (or `//# debugId=`). A match gives module attribution and the module graph; no match still gives chunk-level coverage from the served body.                                                                                                     |
| Report         | `<out>/<variant>/runtime-metrics.json`, `coverage.json`, `profile.json` in perf-kit's shapes (`"paint"` means "at the cut", see `field.cut`), plus `field-report.json`: medians, paired deltas with a seeded bootstrap 90% interval, slow-server outlier pairs.                                                |

**Privacy.** Output goes outside the repo (default `$TMPDIR/perf-kit-field/<site>`).
It holds no cookies, no headers, no page text and no screenshots. IDs, emails and long
numbers in URLs are redacted (add patterns with `redact`). Variant cookies change the
whole profile, so each run starts from the original values and they are restored at the
end, also on Ctrl-C. Block the site's analytics beacons in `block` so runs do not add
synthetic sessions.

**Getting a matching build.** Production rarely serves its sourcemaps. Build the same
commit locally with the deploy's settings and bundler version; identical inputs give
byte-identical chunks. Anything the build fetches at build time must be identical too.
`buildMatch` in `coverage.json` says how much of the page the build explains;
`different-build` means the path exists locally with other content.

### What this mode does NOT do

- **It measures one account, one network, one machine.** Gates, data size and network
  differ per user. Population behavior needs field RUM.
- **Your extensions run in the measured page.** Their scripts show up in `profile.json`
  as `external` rows (the full URL for `chrome-extension://` frames). Use a profile with
  few extensions when CPU numbers matter.
- **No parity check.** Real data changes between loads.
- **Only navigations.** It never clicks or submits. `nonGetRequests` in each
  `runtime-metrics.json` lists what the page itself sent.
- **`--throttle lab` adds the lab throttle on top of the real network.** The real RTT
  stays in, so the result is not comparable to a lab scan.

## Replay (a build that is not deployed)

Field mode needs every variant deployed. Replay gives field-like numbers for a LOCAL
build: record the deployed site once, then serve that recording to a throwaway headless
Chrome with your dist in place of the deployed files.

```bash
# 1. Record one deployed variant (same config file as field mode; Chrome as for field)
node harness.mjs record --config field.json --variant base          # 6 timing runs + 1 body run
# 2. Calibrate: the replay of the RECORDED build must reproduce the field medians
node harness.mjs replay --archive $TMPDIR/perf-kit-replay/<site>-base \
  --dist recorded=/builds/stable/dist --calibrate
# 3. A/B: alternating pairs of two local builds on the same recorded network and server
node harness.mjs replay --archive <dir> --dist base=/builds/stable/dist --dist candidate=./dist
```

### Standalone: one agent records, another replays

`replay.mjs` (bin `perf-kit-replay`) runs record and replay with no harness target and
no perf-kit state. The archive folder is the whole interface: the recording agent hands over
its path, and the replaying agent needs no browser login.

```bash
# Recording agent (the user's Chrome, remote debugging on). No config file needed.
node replay.mjs record --url https://app.example.com/ --cookie version=canary \
  --block analytics.example.com --gate /api/items --ignore-lcp '#splash' \
  --build /builds/stable/dist --settle 6000 --json
# -> stdout: { "archive": "<dir>", "complete": true, "gaps": [], "next": {...}, ... }

# Replaying agent
node replay.mjs inspect --archive <dir> --json            # usable? complete? warnings? age?
node replay.mjs replay --archive <dir> --dist recorded=/builds/stable/dist \
  --calibrate --json --exit-code                          # result.calibration.verdict
node replay.mjs replay --archive <dir> --dist base=/builds/stable/dist --dist candidate=./dist --json
```

- `--json` prints one result object on stdout and sends the log to stderr. An error
  gives `{ "error": "..." }`.
- Exit codes: 0 done, 1 model error, 2 error, 3 record again. Codes 1 and 3 come only
  with `replay --calibrate --exit-code`.
- **Gaps.** `record` and `inspect` compare the archive with the field timing runs. A
  request that at least half of the runs made, that the archive has no response for,
  and that the build does not serve is a gap: the replay answers it with a 404. They
  return `complete` and `gaps`. A gap usually means that the body run stopped too early;
  record again with a longer `--settle`.
- **Calibration verdict.** `calibration.verdict` says what to do next:

  | Verdict        | Meaning                                                   | Next step                                        |
  | -------------- | --------------------------------------------------------- | ------------------------------------------------ |
  | `pass`         | Every row is within tolerance and the archive has no gaps | An A/B on this archive is usable                 |
  | `record-again` | The archive has gaps                                      | Record again (longer `--settle`)                 |
  | `model-error`  | The archive is complete, but a row is off                 | Report the rows. A new recording does not fix it |

- `record` writes `HANDOFF.txt` next to `archive.json`. It holds the next commands
  with absolute paths. `record --out` replaces an earlier archive, and refuses a folder
  that is not empty and holds no `archive.json`.
- `archive.json` has `schemaVersion` and `recordedWith` (tool, Chrome and Node
  versions, cookie names, never values). `inspect` and `replay` refuse an archive with
  another schema or with missing bodies. Record again in that case.
- `inspect` warns when the archive has no network rating, no bandwidth, no named build,
  or is more than 24 h old.
- `--build` on `record` names the local copy of the build that the site serves. The
  replaying agent uses it for the calibration and the `next` commands.
- **Late chunks.** `inspect` runs the chunk discovery check (see "Chunk discovery") on
  the field timing runs with the `--build` manifest. `replay` runs it on each dist's
  replay runs. Both read the CDP requests, not the Resource Timing list, and use the
  archive's `rtt` as the threshold. They return `lateChunksBeforeLcp`, and `replay`
  writes the full result to `runtime-metrics.json` → `chunkDiscovery`. These are leads,
  not predicted gains: a replay A/B decides. The check needs `.vite/manifest.json` that
  is not older than the files it names; copy a build with `mv` or `cp -p`, not `cp -R`.

### How the replay works

| Part                      | What it does                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Archive                   | Every response of the body run with safelisted headers (type, encoding, CORS, `vary`, `location`, `link`; never cookies or auth), request body hashes (never request bodies), the median server time of each call over the timing runs, volatile query parameters (values that change between loads), the field medians, and the timing runs (`timing.json`).                                                                                                                                                                                                        |
| One server for every host | Chrome runs with `--host-resolver-rules`, so the page keeps its real origin (cookies, CORS, absolute URLs unchanged) and no request leaves the machine. HTTPS + HTTP/2, like the recorded site.                                                                                                                                                                                                                                                                                                                                                                      |
| Document                  | The dist's `index.html` plus every tag that only the deployed HTML has (a host's nonce, analytics snippet, service-worker or bot-check loader), after the recorded TTFB. The head comes at once and the rest after the recorded download time, like the recorded early flush. `--document local\|recorded` serves either file bare. An iframe document is served like any recorded response.                                                                                                                                                                         |
| Static files              | From the dist, compressed like the recorded assets (encoding and level fitted on the recorded sizes). Each time to first byte is DRAWN from the recorded TTFB of files that loaded on an idle link (less than 50 kB of other downloads during the wait): a startup step waits for the slowest of its parallel chunks, so a constant median makes every step too fast. Both builds of a pair use the same seed.                                                                                                                                                       |
| Wire queue                | On a busy slow link a response's first byte comes late because of the page's OWN downloads. The replay keeps a FIFO of `wireBytes` (the median extra TTFB of files fetched on a busy link × bandwidth) between the scheduler and the link. Static response headers wait behind the bytes in it, and the body follows them without a second wait. `--wire <kB>` overrides it. Drawing from ALL recorded TTFBs instead counts this queue two times.                                                                                                                    |
| API                       | The recorded response after its recorded server time, matched by method + host + path + query without volatile parameters, then call order (request bodies: exact hash, then normalized hash). A recorded late body sends its last byte after the recorded download time, outside the bandwidth queue (the recorded time already holds that wait). An unrecorded request gets a 404 and is listed in the report.                                                                                                                                                     |
| Bandwidth                 | One token bucket for all streams, most urgent first (`priority: u=N`). A delay per request would let 80 parallel chunks all run at full speed. The default rate is the median over the timing runs of the busiest 250 ms window. On a link whose rate changes (for example a VPN), this estimate is too high; use `--bandwidth <Mbit/s>` and calibrate.                                                                                                                                                                                                              |
| Chrome's network rating   | Over loopback Chrome rates the network as fast. On a slow real network (rating `3g` or slower) it holds back low-priority requests even on HTTP/2, which can add seconds. The replay forces the recorded rating (`navigator.connection.effectiveType`); `--ect` overrides it.                                                                                                                                                                                                                                                                                        |
| Storage                   | The field runs load the page in a profile that visited it before. A page that caches data in `localStorage` (a boot payload, flags) starts from that copy there and does not wait for the API. `--storage warm` first loads the page once, unmeasured, in the same browser context, so the build writes its own storage; the measured load reads it. The HTTP cache stays off and service workers are bypassed on every load, as in the field runs, so only storage and cookies carry over. Default: `warm` when the recorded page had `localStorage`, else `empty`. |
| Calibration               | `--calibrate` compares the first dist's medians with the field medians (LCP, document end, gate start, gate - doc, gate TTFB, after gate). Each must be within max(15%, 150 ms). A row that the field has and the replay lacks (no gate found) is off. The verdict (`pass`, `record-again`, `model-error`) says what to do next; `timing.json` holds the field waterfalls to find a model error.                                                                                                                                                                     |

**Privacy.** The archive holds REAL responses from your account. It goes to
`$TMPDIR/perf-kit-replay/` with owner-only permissions, and `record` refuses a path
inside a git work tree. Delete it when done.

### What replay does NOT do

- **It models one recorded network and server.** A result holds for those conditions;
  record again when the network changes (a slower RTT changes Chrome's rating, and with
  it which requests wait).
- **It does not copy the recorded profile's storage.** With `--storage warm` the build
  writes its own storage in an unmeasured first load. Storage that only older visits or
  other pages write (the archive's `page` field lists the cookie names and the number of
  localStorage keys) is not there. A build that reads it can take another path; the
  report warns when the replay lands on another path than the field.
- **It does not model packet loss.** A lost packet on a real connection holds every
  stream until it is resent. The sampled TTFB covers only part of this.
- **New API calls of a candidate build have no response.** They get a 404 and the
  report lists them; the result may not hold.

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
     guard.fcpObservedInAllRuns && guard.lcpObservedInAllRuns`, and `runtime.cls` must not grow by more than 0.02.
     A faster build that broke a feature is a revert, not a win.
   - **Improvement must beat noise**: check `paintComparison` and `baselineDelta`.
     FCP or LCP must improve by at least max(30ms, 2% of its baseline). Neither
     metric may regress by that amount. Use a full scan with `--no-early-stop`.
5. **Decide**:
   - Accept → `baseline` (re-pin: this is the new reference).
   - Revert → `undefer <name>` → `build`, and don't retry that candidate.
6. **Repeat** from 2. **Converged** when no candidates remain, or 2–3 consecutive
   reverts, or the last accepted improvement is under ~2%.
7. **Compare once**: `scan --parity`. A difference from the original starts recover
   mode — fix or revert until a check of every page is clear; the number you report
   comes from that build.

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
- `app/node_modules/.rolldown/report/dist/` — the build-side report (`output.max_initial_load_bytes`
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
