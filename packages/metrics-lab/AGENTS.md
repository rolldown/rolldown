# metrics-lab — agent contract

You are here to make a page load faster and prove it with measurements. This package
provides the measuring and diagnosis commands; you make the code changes and the
keep/revert decisions.

## Commands (run in this directory)

- `node harness.mjs measure --dist <app>/dist --runs 5 --label <name>` — throttled
  browser runs. Prints LCP in ms (lower = faster) and, once a baseline is pinned, a
  `vs pinned baseline` line with a verdict and a suggested next step.
- `node harness.mjs baseline` — pin the last measurement as the fixed reference every
  later measurement is judged against.
- `node harness.mjs coverage --dist <app>/dist` — per source module: size, and how much
  of it executed at first paint vs by settle. Large modules at ~0% paint are downloaded
  and parsed by the landing page without being used by it — those are your targets.
- Demo app only: `gen`, `build`, `defer <feature>`, `undefer <feature>`, `status`,
  `serve` (see README.md).

## The optimization loop

1. Build the app. `measure --label baseline`. `baseline` (pin it BEFORE changing anything).
2. `coverage` — note the modules that are large but ~0% executed at paint.
3. Read the app's source from its entry file and work out why the landing page loads
   them (follow the import chains).
4. Change the app so the landing page no longer loads them before paint, without
   removing any feature. One small, focused change at a time.
5. Rebuild. Run the app's own functional check (it must pass). `measure --label <change>`.
6. Verdict says "improvement beyond noise" AND the functional check passes → keep the
   change, re-pin with `baseline`, commit. Anything else → revert the change exactly
   and rebuild.
7. Repeat from 2. You are done when coverage reports no candidates left, or two
   attempts in a row were not kept.
8. Report: baseline LCP, final LCP, % change, entry file size before/after, and one
   sentence per kept change.

## Rules

- Judge speed ONLY by the `vs pinned baseline` verdict. Never by the chain
  "vs previous measure" line, never by intuition.
- A faster page that breaks a feature is a failure, not an optimization.
- Never edit this package, the app's build/check scripts, or any thresholds to move
  the numbers.
