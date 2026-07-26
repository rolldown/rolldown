When enabled, Rolldown measures time spent in each plugin hook. If plugins significantly impact build performance, a warning is emitted with a breakdown of plugin timings.

**How it works:**

1. **Minimum build time**: To avoid noisy warnings for fast builds, the warning is only triggered if Rolldown's internal build time (Rust side) exceeds **3 seconds**.

2. **Detection threshold**: A warning is triggered when plugin time (total build time minus link stage time) exceeds 100x the link stage time. This threshold was determined by studying plugin impact on real-world projects.

3. **Identifying plugins**: When the threshold is exceeded, Rolldown reports up to 5 plugins, sorted by their estimated cost, each shown as a share of total build time along with a breakdown of which hooks that time went to. Only plugins with an estimated cost of at least 1 second are included in the report. User callbacks that Rolldown calls directly rather than through a plugin — currently the [`output.advancedChunks`](/reference/OutputOptions.advancedChunks) `groups[].name` classifier — get their own row, listed under `output options`.

4. **Estimating cost**: Hooks are timed with a wall clock, so hooks that run concurrently include time spent waiting behind each other and their durations cannot simply be summed — thousands of concurrent module tasks can inflate a sum far past the time the build actually took, while a hook called one at a time is measured exactly. To make the two comparable, Rolldown measures how long each concurrent phase (module loading, chunk instantiation, `renderChunk`, `augmentChunkHash`) really took, and reports each hook's share of that phase's measured time scaled to the phase's real duration. Reported numbers are therefore shares of wall-clock time, and add up to at most the build's duration.

> [!WARNING]
> These numbers are estimates. Rolldown attributes a phase's entire duration to the hooks that ran in it, so a phase whose cost is actually Rust-side work will still be reported as plugin time — this is why the report stays behind the detection threshold above, which establishes that plugins dominate before the estimate says by how much. Within module loading, hooks that run while few modules are in flight are also under-credited relative to hooks running at peak concurrency.
>
> The measured duration includes Rust-side processing overhead, Tokio async scheduling overhead, NAPI data conversion overhead, and JavaScript event loop overhead, and does not distinguish a hook burning CPU from one awaiting I/O. For hooks using [`this.resolve()`](/reference/Interface.PluginContext#resolve) or [`this.load()`](/reference/Interface.PluginContext#load), the reported time includes waiting for other plugins, which may overestimate that plugin's actual cost.
