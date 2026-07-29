When enabled, Rolldown measures how long your plugin hooks run and warns when they account for a significant share of the build.

**How it works:**

The clock starts and stops **inside the JavaScript callback**, not around the call. Rolldown dispatches most hooks concurrently and they queue up on JavaScript's single thread, so the time between handing a call over and getting a result back is mostly time the call spent waiting. Measuring from inside the callback excludes that wait by construction.

1. **Minimum build time**: to avoid noisy warnings for fast builds, the warning is only triggered if Rolldown's internal build time (Rust side) exceeds **3 seconds**.

2. **Detection threshold**: a warning is triggered when plugin time (total build time minus link stage time) exceeds 100x the link stage time. This threshold was determined by studying plugin impact on real-world projects. The link stage is the one part of a build that runs no plugins at all, which is what makes it a usable baseline.

3. **Rows**: up to 12 hooks are listed, sorted by measured time, each shown as a share of total build time with its call count. Only hooks costing at least 1 second get a line. User callbacks configured on the options rather than on a plugin — `external`, `treeshake.moduleSideEffects`, the file-name and addon callbacks, and the [`output.advancedChunks`](/reference/OutputOptions.advancedChunks) `groups[].name` classifier and `groups[].test` predicate — appear under `input options` / `output options`.

   The headline figure is the wall time in which _any_ plugin callback was running, counted once however much they overlap, so it can never exceed the build. Individual rows can add up to more than it, because one callback may run inside another — `this.emitFile()` in `buildStart` invokes your `assetFileNames`, and that time belongs to both.

> [!IMPORTANT]
> **Some hooks are listed without a number**, under a heading saying they are not measurable.
>
> A span from callback entry to callback exit can be added to another span only if the two never overlap. A synchronous callback cannot overlap — it holds the thread until it returns. An `async` one can: it may suspend at an `await` and let another call of the same hook begin, and then both spans cover the same wall clock and adding them counts it twice. Overlap also changes what the span means — a hook that awaits Rolldown itself, via `this.resolve` or `this.load`, spends most of its span waiting for the bundler, so its elapsed time describes the bundler rather than your plugin.
>
> Rolldown measures the overlap rather than assuming it, and judges its size: it records exactly how much of a hook's total is double counted, and keeps the number when that is under 1% of the span. So a hook dispatched concurrently is still measured exactly when its calls happen not to overlap, and one incidental overlap among thousands of calls does not discard an otherwise good measurement — which also keeps the report stable from run to run rather than dependent on scheduling. A hook that genuinely overlaps itself is named but given no number, because any number would be an upper bound that ranks it above hooks doing more work. To find the real cost of those, profile the JavaScript directly — for example `node --cpu-prof` on your build script, which samples what is actually executing.

**What the listed numbers mean.** Measuring from inside the callback excludes the time a call spent queued; it does not separate running from awaiting. A listed hook is wall time for that callback, not CPU time, so one that awaits I/O without overlapping another call of itself is charged for the wait. The figure excludes the data conversion Rolldown does on either side of the call.

**Not covered.** Plugins written in Rust (`builtin:` and the plugins Rolldown ships internally) have no JavaScript callback to measure and never appear. Neither do parallel plugins, whose hooks run on worker threads. The report is produced when the build is closed, so watch and dev-server rebuilds do not emit it.
