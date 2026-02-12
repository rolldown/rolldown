When enabled, Rolldown measures time spent in each plugin hook. If plugins significantly impact build performance, a warning is emitted with a breakdown of plugin timings.

**How it works:**

1. **Minimum build time**: To avoid noisy warnings for fast builds, the warning is only triggered if Rolldown's internal build time (Rust side) exceeds **3 seconds**.

2. **Detection threshold**: A warning is triggered when plugin time (total build time minus link stage time) exceeds 100x the link stage time. This threshold was determined by studying plugin impact on real-world projects.

3. **Identifying plugins**: When the threshold is exceeded, Rolldown reports up to 5 plugins that take longer than the average plugin time, sorted by duration. Each plugin shows its percentage of total plugin time. Only plugins with total duration of at least 1 second are included in the report.

> [!WARNING]
> For hooks using [`this.resolve()`](/reference/Interface.PluginContext#resolve) or [`this.load()`](/reference/Interface.PluginContext#load), the reported time includes waiting for other plugins, which may overestimate that plugin's actual cost.
>
> Additionally, since plugin hooks execute concurrently, the statistics represent accumulated time rather than wall-clock time. The measured duration also includes Rust-side processing overhead, Tokio async scheduling overhead, NAPI data conversion overhead, and JavaScript event loop overhead.
