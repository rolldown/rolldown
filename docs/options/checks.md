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

Whether to emit warning when detecting plugin timings.

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
