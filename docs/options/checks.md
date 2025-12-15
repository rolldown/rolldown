# Checks Options

- **Type:** `object`
- **Default:** See individual options below

Controls which warnings are emitted during the build process. Each option can be set to `true` (emit warning) or `false` (suppress warning).

## circularDependency

- **Type:** `boolean`
- **Default:** `false`

Whether to emit warning when detecting circular dependency.

## commonJsVariableInEsm

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when detecting common js variable in esm.

## configurationFieldConflict

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when detecting configuration field conflict.

## couldNotCleanDirectory

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when detecting could not clean directory.

## emptyImportMeta

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when detecting empty import meta.

## eval

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when detecting eval.

## filenameConflict

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when detecting filename conflict.

## importIsUndefined

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when detecting import is undefined.

## missingGlobalName

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when detecting missing global name.

## missingNameOptionForIifeExport

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when detecting missing name option for iife export.

## mixedExport

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when detecting mixed export.

## pluginTimings

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when plugins significantly impact build performance.

When enabled, Rolldown measures time spent in each plugin hook. If plugins significantly impact build performance, a warning is emitted with a breakdown of plugin timings.

**How it works:**

1. **Detection threshold**: A warning is triggered when plugin time (total build time minus link stage time) exceeds 100x the link stage time. This threshold was determined by studying plugin impact on real-world projects.

2. **Identifying plugins**: When the threshold is exceeded, Rolldown reports up to 5 plugins that take longer than the average plugin time, sorted by duration. Each plugin shows its percentage of total plugin time.

> [!WARNING]
> For hooks using `ctx.resolve()` or `ctx.load()`, the reported time includes waiting for other plugins, which may overestimate that plugin's actual cost.
>
> Additionally, since plugin hooks execute concurrently, the statistics represent accumulated time rather than wall-clock time. The measured duration also includes Rust-side processing overhead, Tokio async scheduling overhead, NAPI data conversion overhead, and JavaScript event loop overhead.

## preferBuiltinFeature

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when detecting prefer builtin feature.

## unresolvedEntry

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when detecting unresolved entry.

## unresolvedImport

- **Type:** `boolean`
- **Default:** `true`

Whether to emit warning when detecting unresolved import.

## Example

```js
import { defineConfig } from 'rolldown';

export default defineConfig({
  checks: {
    // Enable circular dependency warnings
    circularDependency: true,
    // Disable eval warnings
    eval: false,
    // Enable all other warnings (default)
  },
});
```
