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

## The optimization loop

1. Build the app. `scan --app <appDir>` — the first scan is your baseline.
2. Read EVERY signal class in the scan output; each is a lead with a next-step:
   - **render gap** — paint gated on post-load work. The gate is named when the
     data can name it: a gating fetch/xhr, or heavy pre-paint fonts/images (the
     per-type "before first paint" weights). Fix FIRST: fetches → render with
     bundled defaults and apply results when they land; fonts → paint with one
     preloaded (subset) font, register the rest after paint.
   - **pre-paint CPU by module** — warm-up caches, telemetry, data-module
     evaluation running ahead of paint. Defer what the first render does not need —
     but only judge deferrals AFTER any render gap is fixed (CPU that overlaps a
     blocked render is free, so deferring it can measure worse until then).
   - **defer candidates** — parsed but ~unexecuted at paint: classic lazy-load targets.
   - **pre-paint sibling chunks** — a non-entry chunk fetched AND executed before
     first paint is critical-path transfer, even if every import of it is dynamic.
     Find what runs it at boot (a top-level or render-time `import()`, an eagerly
     mounted component) and move that trigger to the actual interaction.
   - **large modules executed at paint** — "executed" does NOT mean needed; top-level
     data evaluates on import. Verify how much the first render reads; split the rest
     behind a dynamic import.
   - **sibling variant groups** — locales/themes/config families where one variant is
     active per session: keep the default in the entry, load the active one dynamically.
3. Read the app source and find why the landing page pays for each lead.
4. Change the app, without removing any feature. One small change at a time — render
   gap first, then data/variant splits, then CPU deferrals.
5. Rebuild. Run the app's own functional check (it must pass). `scan`.
6. "improvement beyond noise" AND the check passes → keep it, `scan --pin` (or
   `baseline`), commit. Anything else → revert the change exactly and rebuild.
   (A deferral that measures worse while a render gap exists is worth retrying
   after the gap is fixed.)
7. Repeat. Declare done ONLY when the verdict reports every signal class clear and
   fresh — never because a single report looks empty. One tool's silence only means
   that tool sees nothing; the verdict checks them all and tells you which signals
   you haven't gathered yet.
8. Report: baseline LCP, final LCP, % change, entry size before/after, and one
   sentence per kept change.

## Rules

- Judge speed ONLY by the `vs pinned baseline` verdict. Never by the chain
  "vs previous measure" line, never by intuition.
- A faster page that breaks a feature is a failure, not an optimization.
- Never edit this package, the app's build/check scripts, or any thresholds to move
  the numbers.
