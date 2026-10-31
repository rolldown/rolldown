# perf-kit — agent contract

You are here to make a page load faster and prove it with measurements. This package
provides the measuring and diagnosis commands; you make the code changes and the
keep/revert decisions.

All commands work from any working directory (`node <this dir>/harness.mjs ...`).
The first `--app <appDir>` is remembered, so afterwards every command can be run
bare — no paths. Each target app keeps its own state (baseline, history, reports).

Installed from npm, perf-kit runs as `npx <package name> ...` with no install into the
project; `help` prints the exact spelling and the path of this file. The state then
goes to `.perf-kit/` under the directory you run it from: keep that directory out of
commits. In the repository, `packages/rolldown` sits next to perf-kit and is the default
source of `rolldown-install`. An installed copy has no such neighbour, so pass
`--from <packages/rolldown of a checkout built with just build-rolldown-release>`
(see the Worker section).

## The two commands that matter

- `node harness.mjs scan --app <appDir>` — one browser session that does it all:
  N throttled timed runs (FCP and LCP vs the pinned baseline), first-paint
  coverage, boot CPU profile — then prints the fused verdict. The FIRST scan of a
  target automatically pins the baseline. After a kept change, `scan --pin` re-pins.
- `node harness.mjs verdict` — re-print the fused OPEN / clear / UNKNOWN checklist
  for the gathered signals, with staleness tracking against the current build.
  **This is the only "done" that counts**: it refuses to conclude while a lead is
  open or a signal is missing/stale, and even its all-clear states what the tools
  cannot see.

Individual commands exist too (`measure [--pin]`, `coverage`, `profile`,
`baseline`, `target [<appDir>] [--demo]`) plus demo-app helpers (README.md).

The throttle is part of the measurement identity. The first scan calibrates a bandwidth
scale (x1 to x8, with RTT 150 ms and a 4x CPU slowdown) and pins it. To judge the page on
the network of a deployed environment instead, pin that network:
`scan --bandwidth <Mbit/s> --rtt <ms> --cpu <rate>`, with the values that a calibrated
`record` measured (`--cpu 1` = this machine's CPU). A change of either makes the pinned
baseline incomparable: re-pin on the unchanged build before you judge a change.

On rolldown devtools report builds (vite ≥ 8:
`build.rolldownOptions.devtools = { mode: "report" }`) several static queries skip the
import-chain archaeology entirely: `graph` ranks split candidates by **retained size**
(the dominator subtree a deferral removes from the initial load, with `via` naming the
one import chain to cut); `what-if <module> [<module>...] [--keep a,b]` lists the exact
modules and bytes one deferral frees (sentries in `--keep` stay eager) — pass **two or
more** modules and it prices them as one combined plan against the summed-individually
total, so shared internals dominated by no single module (a library split across imports,
sibling routes) show up as combined-exceeds-the-sum; `cut <module>` returns the **fewest
import statements** to make dynamic to detach a module reached by several paths (the
last-hop edges nearest it, the exact files to edit — a min edge cut, which dominator
retained-size can't answer once several paths reach the module; `--keep` edges are
protected; `--root a,b` keeps class modules eager with their closure so the cut lands
below them; on `what-if` and `cut`, `--root` also takes a module that only a dynamic import
reaches but the app loads on every start (a boot chain of `import()` calls): it joins the
entries, so both commands price that startup load); and `graph-diff [--against <file>]`
reports, deterministically and with no
browser run, what a change did to the initial load (the modules that ENTERED or LEFT the
eager set with byte deltas) against the pinned baseline graph or any saved sidecar. Rank
candidates statically with these, then verify the winner's real LCP effect with `scan`.
`scan`/`verdict` fold the graph in as the **statically retained imports** signal;
when the app builds with rolldown but the graph was never collected, the verdict
reports it UNKNOWN with the one config line that enables it — enable it and rebuild
rather than justifying the gap: it is the cheapest signal in the whole kit (no
browser run, and it prices every candidate at once).

The report folder, `node_modules/.rolldown/report/<output dir>`, carries its own guide.
The static commands print the folder path. Its `AGENTS.md` lists the files, the metric ids
and the delta workflow; its `entry.md` is the summary with a file index. Read them when
you need a number the kit does not print, such as hook time per plugin or the build
history.

## If the target is a server, not a page

`node-scan --entry <builtServerScript> --ready <spec>` measures a process coming up
instead of a page loading. The loop below is unchanged — baseline, one change, measure,
accept or revert — with three differences that decide whether your work is worth anything:

- **Declare readiness once and never change it.** `--ready port:3000` /
  `'stdout:listening on'` / `http://127.0.0.1:3000/health` / `exit`. It is pinned with the
  target; two scans that measured different moments are not comparable, the same way two
  browser scans under different net scales are not.
- **Read the ownership line before doing any work.** `node-scan` reports pre-ready CPU
  split into app modules / runtime+native / engine. When app modules hold a small share,
  the bundle is not the cost — startup is native addon loading, TLS/crypto init, or a
  DB handshake — and deferring imports will not move the number no matter how good the
  deferral is. Say that and stop, rather than shipping edits that cannot help.
- **Judge small startups by bytes, not milliseconds.** Readiness polling quantizes; when
  `node-scan` prints the resolution note, the millisecond delta is not evidence and the
  cold-at-ready byte count is.

`graph`, `what-if`, `cut` and `graph-diff` work exactly as below on a server build —
a top-level import and a render-blocking import are the same edge in the module graph.
Bun is not supported (JavaScriptCore, not V8) and edge runtimes can only be analyzed
statically; do not report a measured number for either.

## If the target is a Cloudflare Worker

`worker-scan --app <appDir>` measures the time a fresh isolate spends in the Worker's
global scope, under the app's own workerd, and attributes it to modules. Build the app
first with the config `worker-config` writes (the report on every environment, sourcemaps
on the worker). The loop below is unchanged, with these differences:

- **Startup is the global scope.** `worker.app_startup_ms` is the import of the main module
  and its static closure, timed from the host minus an empty-import floor. Isolate boot
  sits outside it. `worker-scan` against the pinned baseline is the keep-or-revert signal.
  Cloudflare's startup limit counts the same scope; the number to quote in a PR comes
  from `wrangler check startup` on the same build, before and after.
- **A deferral moves work, it does not delete it.** The first request that needs a deferred
  module pays its load, once per isolate. Pin the routes that matter with `--request` and
  read the "cost MOVED here" line next to the startup gain. perf-kit does not know the
  route mix; an isolate whose first request needs the module gains nothing. Agent chats and
  RPC run over WebSockets: pin them as `--request 'ws:/path|<message>'`, with a message that
  reaches the deferred code (an RPC call, a chat turn).
- **Durable Object and Workflow classes are roots.** The runtime loads them by name, so their
  modules stay eager with their whole closure. `cut <module> --root <class modules>` lands
  the cut below them; without `--root`, every cut points at the entry's class exports,
  which cannot become dynamic. The scan prints the class names from `wrangler.json`.
- **Three kinds of edge, three kinds of fix.** `why <module>` names the importers. An importer
  under `node_modules` is upstream: when the library is only used in async code, a lazy shim
  behind a Vite alias is the app-side fix. A module reached only from a synchronous
  constructor or getter cannot be deferred by a dynamic import: say so and move on. A mixed
  `import { type X } from 'm'` keeps a side-effect import of `m` under `verbatimModuleSyntax`:
  change it to `import type`, no dynamic import needed.
- **Library top-level work is charged by the attribution build.** When the profile leaves
  milliseconds in "(unmapped)" or in zod, run `worker-attrib` on the `ROLLDOWN_PERF_KIT_ATTRIB=1`
  build: it names the module whose evaluation runs the library code, with the import chain.
- **The report needs a rolldown with the report mode.** The npm release has none, so with it
  no build writes `module-graph.json` and `graph`, `what-if`, `cut`, `why` and `graph-diff`
  have nothing to read. `worker-config` names the app's rolldown when it lacks the mode;
  `rolldown-install --app <appDir>` copies a built package over the copy the app's vite
  resolves: the one next to perf-kit in a rolldown checkout, or `--from <packages/rolldown>`
  of a checkout built with `just build-rolldown-release` (an installed perf-kit has no package
  next to it, so `--from` is required there). The swap is not a config change;
  `<package manager> install --force` restores the release. Rebuild and re-pin the
  baseline after it: the bundler changed.
- **Read the two delta lines.** `worker-scan` prints `vs pinned baseline` (every change since
  the pin, added up) and `vs previous scan` (the last change alone). Judge a change by the
  second line, or re-pin with `--pin` after each kept change so the first line means the same.
- **The pinned routes must answer 2xx.** A 404 or 5xx never ran the handler, so no moved load
  can show on it, and the scan says so. Pin routes the app serves. Do not invent a `.dev.vars`
  to get past a 5xx: without the real variables the first-request numbers stay invalid.
- **The functional check is the app's own test suite, run the way the repo documents it.** When
  the test pool fails for environment reasons (port exhaustion, `EADDRNOTAVAIL`), run it serially
  or in batches. Never reinstall dependencies inside the loop: that undoes `rolldown-install`,
  and the config shim then refuses to build. Never move or symlink packages inside `node_modules`.
- **Coverage comes from inside the isolate** and needs Wrangler 4.108.0 or later. When the
  scan reports it unavailable, pass `--tools <dir>` with a newer wrangler for that run, or
  work from the profile and the static graph; do not add the compatibility flags to the app.
- **Never upload, never deploy, never edit the app's config, variables or bindings to satisfy
  the tool.** A 5xx on the first requests usually means the app needs its `.dev.vars`; startup
  is unaffected, the first-request numbers are not valid until it has them.

## If the lab and production disagree

`field --config <file>` measures the DEPLOYED site in the user's own logged-in Chrome
(README.md, "Field mode"). It does not replace the loop above; it checks where the lab
cannot see:

- **Read the critical path first.** When `after gate` is small and `gate ttfb` is most
  of the LCP, the page waits on the server; only `gate start` (client time before the
  request the LCP waits for) can move with a bundle change. Optimize what delays that
  request, not what renders after it.
- **Judge by the paired deltas and their 90% intervals,** never by one load or by a
  median of unpaired runs. An interval that crosses 0 is "no measured difference".
- **Compare `gate - doc`, not only `gate start`,** when the variants come from different
  servers (a preview worker can answer the HTML hundreds of ms slower than production).
- **Module rows need a build match.** Read `buildMatch` before quoting a module; with no
  match only the chunk-level rows are evidence.
- Field output contains real account traffic. Keep it where the tool writes it; never
  copy it into a repo, a ticket or a merge request.

`record` + `replay` give field-like numbers for a build that is NOT deployed (README.md,
"Replay"):

- **Calibrate before any A/B.** `replay --calibrate` on the recorded build must PASS.
  A FAIL means the model misses something real; find it in `timing.json` (the field
  waterfalls) before you quote a replay delta.
- **Read the unrecorded requests.** A page request that the recording never saw gets a
  404; a candidate that adds an API call is then not measured as it would run.
- **Record again when the network changes.** Chrome's network rating decides which
  requests wait; an archive from a fast network says nothing about a slow one.
- The archive holds real responses. It stays in `$TMPDIR`; never copy it anywhere.
- **Two agents: hand over the archive path only.** The recording agent runs
  `node replay.mjs record ... --json` and passes the `archive` path. The replaying
  agent runs `node replay.mjs inspect --archive <dir> --json` first, then
  `replay ... --calibrate --json --exit-code`. Exit 2 or `usable: false` means record
  again; do not edit `archive.json` by hand.
- **Act on the calibration verdict, not on PASS/FAIL alone.** `record-again` (exit 3,
  `gaps` not empty): record again with a longer `--settle`, at most twice.
  `model-error` (exit 1): the archive is complete; do NOT record again, report the rows
  that are off. Recording again cannot fix a model error.
- **Keep `--storage warm` for a page that caches data in storage.** The field profile
  has visited the page before; an empty context measures a first visit instead.
- **Find leads in the field runs, decide with a replay A/B.** `inspect` →
  `lateChunksBeforeLcp` lists static chunks that started more than one recorded RTT
  after a static ancestor finished, in the real field runs. Check in the source whether
  paint needs them before you change a chunk boundary or a preload. Never quote a
  discovery delay as a gain.

## The rendered output must not change

The first scan of a target saves the build as the ORIGINAL — run it before you touch
any code, and pin the client-side routes your work can reach with it:
`scan --app <appDir> --pages /pricing,/settings` (entry HTML files are found
automatically; a shared chunk or stylesheet you change for `/` can break another page).

- **Optimize freely** — nothing is compared per change. Each scan only says whether the
  LCP element is still the original's; when it is not, LCP is timing a cheaper paint
  (a hidden hero, a placeholder) and that gain is probably not real — find out why
  before you build on it.
- **When you have finished: `scan --parity`.** It takes the final measurement and
  compares every pinned page with the original: first-screen and full-page pixels, page
  height, visible text, accessible names, new runtime errors, the LCP element. The
  verdict holds it as PENDING until then; you are not done without it.
- **A difference starts RECOVER MODE**: that build's LCP numbers do not count until it
  matches again — often it was faster BECAUSE it broke (a stylesheet split, reordered or
  made non-blocking without the swap; a font deferred until it never loads). The output
  names what changed — the element and the property or font, the missing text, the new
  error — and points at a diff image. Fix the change or revert it, and choose how you
  re-check: `parity strategy each-tweak` (every scan re-checks and records a snapshot)
  or `on-demand` (you run `parity`; `--only /page` re-checks just the broken pages).
  `parity recover` lists the options, including bisecting your kept commits with
  `git bisect run` and `parity --exit-code`; `parity history` shows every check. A fix
  may give back part of the gain — report the number of the build that matches.
- **Never** run `parity approve`, `parity anchor --replace`, or add a `--mask`, to make a
  difference go away. Those are human decisions and every verdict lists them. If you
  believe a difference is intended, stop and say so, with the evidence path.
- UNKNOWN there means the original itself renders differently between two loads (a
  human adds masks) or the LCP element was not compared yet (scan).

## The optimization loop

1. Build the app. `scan --app <appDir>` — the first scan is your baseline and saves
   the original build.
2. Read EVERY signal class in the scan output; each is a lead with a next-step:
   - **render-blocking CSS gate** — FCP cannot precede the last render-blocking
     stylesheet; when they land together, CSS is the paint gate and no amount of
     JS work will move it. Fix FIRST, alongside render gap: inline the small
     critical CSS, load the rest non-blocking (preload + media swap), split
     styles only later routes need. (On pages loading many scripts, a plain
     `media="print"` link gets fetch-deprioritized behind them — pair it with
     `rel="preload"`.)
   - **late chunk candidates before FCP or LCP** — inspect
     `runtime-metrics.json` → `chunkDiscovery.fcp.groups` and
     `chunkDiscovery.lcp.groups`. Each check follows static imports that start
     more than one RTT after an ancestor finishes. The download must finish before
     that paint. The check uses `chunk-graph.json` or the current Vite manifest.
     Each example includes an import chain, request timings, and the available
     source module and initiator. Request details also record Chrome's initial
     priority and priority changes. Check them when testing preload priorities.
     Repeated URLs and redirects omit ambiguous network evidence.
     Follow the available source locations to the source. Check whether the
     chunk renders the first visible content or the LCP element. The report marks
     this dependency `unverified`; timing alone cannot prove it. If paint needs
     the chunk, test an earlier preload or a different chunk boundary. If paint
     does not need it, test deferring its load and work. Groups and paint windows
     overlap. Never add their bytes or delays. A delay is not a predicted paint gain.
   - **render gap** — LCP after the load event. Downloads can continue after
     load. Inspect concurrent script, fetch/XHR, font, and image evidence together.
     One candidate does not exclude another. Check which responses rendering
     awaits before changing load order. Test moving optional data reads after
     paint. Keep the LCP image request early. Check which fonts visible text needs.
     Compare the candidates with the CPU profile. When these checks do not
     explain the gap, check whether the LCP element **mounts invisible**: an entry
     animation or fade-in wrapper that starts the hero at opacity 0 delays LCP
     until the reveal — LCP counts the first frame the element paints VISIBLE.
     Render the hero visible immediately and animate only decoration.
   - **pre-paint CPU by module** — the profile maps all sampled same-origin
     chunks through their sourcemaps, including dynamic imports. Inspect source
     rows and their chunk filenames in `profile.json`. Missing maps, unmapped
     positions, inline scripts, external scripts and mixed top-level lines
     (`top-level`) stay unattributed. Do not
     classify a generated filename as engine work. Old entry-only profiles must
     be collected again. The profile stops at FCP observation; its polling tail
     can include post-paint work. It does not cover the FCP-to-LCP interval.
     Sampled time does not prove that work blocks paint or can be deferred.
     Check warm-up caches, telemetry, and data-module evaluation. Defer what the first render does not need —
     but only judge deferrals AFTER any render gap is fixed (CPU that overlaps a
     blocked render is free, so deferring it can measure worse until then).
   - **cold bytes at paint** — the unified byte view: weight fetched+parsed before
     first paint but mostly unread by it, coldest first. Its middle band is the one
     that hides everywhere else: a vendor SDK executing 10–50% at boot (firestore,
     an analytics kit) is neither a "candidate" nor "large executed", yet one
     boot-time init call is dragging the whole package. Defer the CALL, not just
     the import. Framework runtimes (react-dom) are annotated — their cold bytes
     rarely move; don't chase them.
   - **defer candidates** — parsed but ~unexecuted at paint: classic lazy-load targets.
   - **pre-paint sibling chunks** — a non-entry chunk fetched AND executed before
     first paint is critical-path transfer, even if every import of it is dynamic.
     Find what runs it at boot (a top-level or render-time `import()`, an eagerly
     mounted component) and move that trigger to the actual interaction.
   - **static pre-paint transfer** — chunks fetched before first paint (static
     script tags, modulepreloads) but executed only after it. Execution timing
     says "deferred"; the download still competed with the paint for bandwidth,
     and when transfer dominates LCP this is the biggest lever. Load them on
     demand: dynamic import, or drop them from the initial tags/preloads.
   - **large modules executed at paint** — "executed" does NOT mean needed; top-level
     data evaluates on import. Verify how much the first render reads; split the rest
     behind a dynamic import.
   - **sibling variant groups** — locales/themes/config families where one variant is
     active per session: keep the default in the entry, load the active one dynamically.
   - **statically retained imports** (rolldown builds only) — this view starts
     at build entries. It does not price dynamic imports that startup immediately
     executes. Check coverage and CPU evidence for those imports, and pass the modules
     they load as `--root` to `what-if` / `cut` to price that startup load.
     The module graph's
     dominator view: any non-framework module retaining ≥100KB on the initial load
     is a priced split candidate; `what-if` shows the exact cut, `cut` the fewest
     import edits when several paths reach it, and `what-if <a> <b>` the combined
     free when a package repeats across rows. Retained is potential, not proof — the
     first render may genuinely need it; if so, say why.
3. Read the app source and find why the landing page pays for each lead.
4. Change the app, without removing any feature. One small change at a time — render
   gap first, then data/variant splits, then CPU deferrals.
5. Rebuild. Run the app's own functional check (it must pass). `scan --no-early-stop` — or
   `scan --quick` (1 run, no profile) as a cheap probe on slow apps; quick results
   are indicative only. Early-stopped scans are probes too. Check both paint
   metrics with a full scan of at least 3 runs before a keep/revert decision.
6. Read `paintComparison` and both entries in `baselineDelta`. At least one of
   FCP or LCP must improve beyond noise. Neither may regress beyond noise.
   The threshold for each metric is max(30ms, 2% of its baseline).
   Both metrics must appear in every run. If these checks and the functional check pass,
   keep the change, `scan --pin` (or `baseline`), and commit. Otherwise, revert
   the change and rebuild. Keep the same content and features.
   (A deferral that measures worse while a render gap exists is worth retrying
   after the gap is fixed.)
7. Repeat. When you have finished optimizing, run `scan --parity` (above) and work any
   difference in recover mode. Declare done ONLY when the verdict reports every signal
   class clear and fresh — never because a single report looks empty. One tool's
   silence only means that tool sees nothing; the verdict checks them all and tells
   you which signals you haven't gathered yet.
8. Report: baseline and final FCP and LCP, their % changes, entry size before/after, and one
   sentence per kept change — **plus the final verdict checklist, copied verbatim.**
   A re-pinned baseline records a gain; it does NOT close the checklist. If you stop
   while leads are OPEN, say "stopping with N lead(s) OPEN" and justify each one:
   a measurement (you tried it, the delta was sub-noise) or a concrete constraint
   (framework dep, the first paint genuinely needs it, outside your declared scope).
   Never describe OPEN leads as "confirmed done" — misquoting the verdict is the
   one way to fail this task even with a good number.

## Rules

- Judge speed ONLY by the `vs pinned baseline` verdict. Never by the chain
  "vs previous measure" line, never by intuition.
- A faster page that breaks a feature, or renders differently from the original, is
  a failure, not an optimization.
- Never edit this package, the app's build/check scripts, or any thresholds to move
  the numbers.
- **The functional check is the contract — never modify it.** If it fails after your
  change, your change is wrong: fix the app or revert. A check you edited proves
  nothing.
- **Never change the app's build layout (outDir, file names) to satisfy this tool.**
  Aim the tool at the app instead: `--app <appRoot>` resolves dist/build/out
  automatically, `--dist <builtDir>` aims at any output dir directly.
