When enabled, Rolldown measures how long your plugin hooks run and warns when they account for a significant share of the build.

**How it works:**

The clock starts and stops **inside the JavaScript callback**, not around the call. Rolldown dispatches most hooks concurrently and they queue up on JavaScript's single thread, so the time between handing a call over and getting a result back is mostly time the call spent waiting. Measuring from inside the callback excludes that wait by construction.

1. **Minimum window**: nothing is reported unless plugin callbacks ran across a window of at least **3 seconds**, so ordinary fast builds stay quiet.

2. **Detection threshold**: a warning is emitted when plugin JavaScript held the thread for at least **20%** of that window. This is measured as the union of every callback's span, so overlapping calls are counted once and the figure can never exceed the window.

3. **Rows**: up to 12 hooks are listed, sorted by measured time, each shown as a share of the window with its call count. Only hooks costing at least 1 second get a line. User callbacks configured on the output options rather than on a plugin — the [`output.advancedChunks`](/reference/OutputOptions.advancedChunks) `groups[].name` classifier and its `groups[].test` predicate — appear under `output options`.

> [!IMPORTANT]
> **Some hooks are listed without a number**, under a heading saying they are not measurable.
>
> A span from callback entry to callback exit can be added to another span only if the two never overlap. A synchronous callback cannot overlap — it holds the thread until it returns. An `async` one can: it may suspend at an `await` and let another call of the same hook begin, and then both spans cover the same wall clock and adding them counts it twice. Overlap also changes what the span means — a hook that awaits Rolldown itself, via `this.resolve` or `this.load`, spends most of its span waiting for the bundler, so its elapsed time describes the bundler rather than your plugin.
>
> Rolldown counts overlap per hook rather than assuming it. A hook whose calls never overlapped is measured exactly, even when Rolldown dispatched it concurrently. A hook whose calls did overlap is named but given no number, because any number would be an upper bound that ranks it above hooks doing more work. To find the real cost of those, profile the JavaScript directly — for example `node --cpu-prof` on your build script, which samples what is actually executing.

**What the listed numbers mean.** A listed hook is wall time for that callback, not CPU time: a hook that awaits I/O without ever overlapping another call of itself is charged for the wait. The figure excludes the data conversion Rolldown does on either side of the call.

**Not covered.** Plugins written in Rust (`builtin:` and the plugins Rolldown ships internally) have no JavaScript callback to measure and never appear. Neither do parallel plugins, whose hooks run on worker threads. The report is produced when the build is closed, so watch and dev-server rebuilds do not emit it.
