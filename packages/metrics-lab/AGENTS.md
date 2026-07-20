# metrics-lab — agent contract

You are here to make a page load faster and prove it with measurements. This package
provides the measuring and diagnosis commands; you make the code changes and the
keep/revert decisions.

All commands work from any working directory (`node <this dir>/harness.mjs ...`).
The first `--app <appDir>` is remembered, so afterwards every command can be run
bare — no paths. Each target app keeps its own state (baseline, history, reports).

## The two commands that matter

- `node harness.mjs scan --app <appDir>` — one browser session that does it all:
  N throttled timed runs (LCP + verdict vs the pinned baseline), first-paint
  coverage, boot CPU profile — then prints the fused verdict. The FIRST scan of a
  target automatically pins the baseline. After a kept change, `scan --pin` re-pins.
- `node harness.mjs verdict` — re-print the fused OPEN / clear / UNKNOWN checklist
  for the gathered signals, with staleness tracking against the current build.
  **This is the only "done" that counts**: it refuses to conclude while a lead is
  open or a signal is missing/stale, and even its all-clear states what the tools
  cannot see.

Individual commands exist too (`measure [--pin]`, `coverage`, `profile`,
`baseline`, `target [<appDir>] [--demo]`) plus demo-app helpers (README.md).

On rolldown devtools-metrics builds (vite ≥ 8:
`build.rolldownOptions.devtools = { mode: "metrics" }`) several static queries skip the
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
protected); and `graph-diff [--against <file>]` reports, deterministically and with no
browser run, what a change did to the initial load (the modules that ENTERED or LEFT the
eager set with byte deltas) against the pinned baseline graph or any saved sidecar. Rank
candidates statically with these, then verify the winner's real LCP effect with `scan`.
`scan`/`verdict` fold the graph in as the **statically retained imports** signal;
when the app builds with rolldown but the graph was never collected, the verdict
reports it UNKNOWN with the one config line that enables it — enable it and rebuild
rather than justifying the gap: it is the cheapest signal in the whole kit (no
browser run, and it prices every candidate at once).

## The optimization loop

1. Build the app. `scan --app <appDir>` — the first scan is your baseline.
2. Read EVERY signal class in the scan output; each is a lead with a next-step:
   - **render-blocking CSS gate** — FCP cannot precede the last render-blocking
     stylesheet; when they land together, CSS is the paint gate and no amount of
     JS work will move it. Fix FIRST, alongside render gap: inline the small
     critical CSS, load the rest non-blocking (preload + media swap), split
     styles only later routes need. (On pages loading many scripts, a plain
     `media="print"` link gets fetch-deprioritized behind them — pair it with
     `rel="preload"`.)
   - **render gap** — paint gated on post-load work. The gate is named when the
     data can name it: a gating fetch/xhr, or heavy pre-paint fonts/images (the
     per-type "before first paint" weights). Fix FIRST: fetches → render with
     bundled defaults and apply results when they land; fonts → paint with one
     preloaded (subset) font, register the rest after paint. When neither the
     fetches nor the profile explain the gap (profile says framework/baseline
     work), check whether the LCP element **mounts invisible**: an entry
     animation or fade-in wrapper that starts the hero at opacity 0 delays LCP
     until the reveal — LCP counts the first frame the element paints VISIBLE.
     Render the hero visible immediately and animate only decoration.
   - **pre-paint CPU by module** — warm-up caches, telemetry, data-module
     evaluation running ahead of paint. Defer what the first render does not need —
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
   - **statically retained imports** (rolldown builds only) — the module graph's
     dominator view: any non-framework module retaining ≥100KB on the initial load
     is a priced split candidate; `what-if` shows the exact cut, `cut` the fewest
     import edits when several paths reach it, and `what-if <a> <b>` the combined
     free when a package repeats across rows. Retained is potential, not proof — the
     first render may genuinely need it; if so, say why.
3. Read the app source and find why the landing page pays for each lead.
4. Change the app, without removing any feature. One small change at a time — render
   gap first, then data/variant splits, then CPU deferrals.
5. Rebuild. Run the app's own functional check (it must pass). `scan` — or
   `scan --quick` (1 run, no profile) as a cheap probe on slow apps; quick results
   are indicative only, so confirm any accept/revert/pin with a full scan.
6. "improvement beyond noise" AND the check passes → keep it, `scan --pin` (or
   `baseline`), commit. Anything else → revert the change exactly and rebuild.
   (A deferral that measures worse while a render gap exists is worth retrying
   after the gap is fixed.)
7. Repeat. Declare done ONLY when the verdict reports every signal class clear and
   fresh — never because a single report looks empty. One tool's silence only means
   that tool sees nothing; the verdict checks them all and tells you which signals
   you haven't gathered yet.
8. Report: baseline LCP, final LCP, % change, entry size before/after, and one
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
- A faster page that breaks a feature is a failure, not an optimization.
- Never edit this package, the app's build/check scripts, or any thresholds to move
  the numbers.
- **The functional check is the contract — never modify it.** If it fails after your
  change, your change is wrong: fix the app or revert. A check you edited proves
  nothing.
- **Never change the app's build layout (outDir, file names) to satisfy this tool.**
  Aim the tool at the app instead: `--app <appRoot>` resolves dist/build/out
  automatically, `--dist <builtDir>` aims at any output dir directly.
