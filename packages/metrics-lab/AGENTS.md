# metrics-lab — agent contract

You are here to make a page load faster and prove it with measurements. This package
provides the measuring and diagnosis commands; you make the code changes and the
keep/revert decisions.

## Commands (run in this directory)

- `node harness.mjs measure --dist <app>/dist --runs 5 --label <name>` — throttled
  browser runs. Prints LCP (lower = faster), a `vs pinned baseline` verdict once a
  baseline is pinned, and two extra flags when they apply: a **render gap** (paint
  landed long after `load`) and **pre-paint CPU** (long tasks before first paint).
- `node harness.mjs baseline` — pin the last measurement as the fixed reference every
  later measurement is judged against.
- `node harness.mjs coverage --dist <app>/dist` — per source module: size and how much
  executed at first paint vs by settle. Prints up to three sections; every one is a
  lead (see "reading the signals").
- `node harness.mjs profile --dist <app>/dist` — boot CPU by source module, from
  navigation to first paint. Use it whenever measure reports pre-paint CPU.
- Demo app only: `gen`, `build`, `defer <feature>`, `undefer <feature>`, `status`,
  `serve` (see README.md).

## Reading the signals — every finding class and the move it suggests

1. **Render gap** (measure): first paint landed well after `load` — rendering is
   gated on post-load work, usually an `await` on a fetch (config, data, locale)
   before the first render. The gating fetches are listed by name. Move: render
   immediately with bundled defaults and apply the fetched result when it arrives.
   **Fix this class before judging CPU deferrals.**
2. **Pre-paint CPU** (measure → profile): modules burning CPU before paint —
   warm-up caches, telemetry/fingerprinting, module evaluation of big data. Move:
   defer work the first render does not need (idle callback and/or dynamic import).
   Caveat: CPU that overlaps a render-blocking fetch is free — if a render gap
   exists, fix it first or the deferral will measure worse and you will wrongly
   revert it.
3. **Defer candidates** (coverage): sizeable modules parsed but ~unexecuted at
   paint — classic lazy-load targets (routes, below-fold features).
4. **Large modules executed at paint** (coverage): "executed" does NOT mean needed —
   top-level data counts as executed the moment its module is imported. Move: check
   how much the first render actually reads; split rarely-read parts (full records,
   bodies, alternate variants) behind a dynamic import.
5. **Sibling variant groups** (coverage): families of same-shaped modules (locales,
   themes, per-tenant configs) all evaluated while one variant is active per
   session. Move: keep the default in the entry, load the active variant dynamically.

## The optimization loop

1. Build the app. `measure --label baseline`. `baseline` (pin BEFORE changing anything).
2. Collect every lead: the measure flags (render gap, pre-paint CPU → `profile`) and
   all coverage sections. An empty defer-candidates list does NOT mean you are done —
   check the other four classes.
3. Read the app source and find why the landing page pays for each lead.
4. Change the app so it stops paying, without removing any feature. One small,
   focused change at a time, in this order when applicable: render gap first, then
   data/variant splits, then CPU deferrals.
5. Rebuild. Run the app's own functional check (it must pass). `measure --label <change>`.
6. Verdict says "improvement beyond noise" AND the functional check passes → keep the
   change, re-pin with `baseline`, commit. Anything else → revert the change exactly
   and rebuild. (If a deferral measures worse while a render gap exists, revisit it
   after the gap is fixed.)
7. Repeat from 2. You are done when the signals are clean, or two attempts in a row
   were not kept.
8. Report: baseline LCP, final LCP, % change, entry size before/after, and one
   sentence per kept change.

## Rules

- Judge speed ONLY by the `vs pinned baseline` verdict. Never by the chain
  "vs previous measure" line, never by intuition.
- A faster page that breaks a feature is a failure, not an optimization.
- Never edit this package, the app's build/check scripts, or any thresholds to move
  the numbers.
