# External Modules

When a module is marked as external, Rolldown will not bundle it. Instead, the `import` or `require` statement is preserved in the output, and the module is expected to be available at runtime.

```js
// input
import lodash from 'lodash';
console.log(lodash);

// output (lodash is external)
import lodash from 'lodash';
console.log(lodash);
```

This page explains how externals work end-to-end: how a module becomes external, how its import path is determined in the output, and how the relevant options and plugin hooks interact.

## How a Module Becomes External

There are three ways a module can be marked as external:

1. **The [`external`](/reference/InputOptions.external) option** — a config-level pattern (string, regex, array, or function) that tests each import specifier. See the [option reference](/reference/InputOptions.external) for pattern syntax, examples, and caveats.

2. **A plugin's `resolveId` hook** — a plugin can return `{ id, external: true }` (or `"relative"` / `"absolute"`) to explicitly mark a module as external. A plugin can also `return false` to mark the raw specifier as external with the same normalization as the `external` option.

3. **Unresolved modules** — if no plugin or the internal resolver can find a module and the `external` option matches the specifier, Rolldown treats it as external rather than throwing an error.

## The Full Resolution Flow

Here is the step-by-step process Rolldown follows when it encounters an import:

### 1. First `external` check

The raw import specifier (e.g. `'./utils'`, `'lodash'`) is tested against the [`external`](/reference/InputOptions.external) option with `isResolved: false`. If it matches, the module is marked as external immediately — **plugins and the internal resolver are skipped entirely**.

### 2. Plugin `resolveId`

If the first check did not match, plugins get a chance to resolve the import:

| Plugin return value                   | Effect                                                                            |
| ------------------------------------- | --------------------------------------------------------------------------------- |
| `return false`                        | External. Uses the raw specifier as the module ID (same normalization as step 1). |
| `return { id, external: true }`       | External. Uses `id` as the module ID.                                             |
| `return { id, external: "relative" }` | External. Path is **always** relativized (overrides config).                      |
| `return { id, external: "absolute" }` | External. Path is **always** kept verbatim (overrides config).                    |
| `return { id }` (no `external`)       | Resolved, continue to step 3 with the resolved ID.                                |
| `return null`                         | No plugin handled it, fall through to step 3.                                     |

### 3. Internal resolver

Rolldown's built-in resolver tries to find the module on disk.

### 4. Second `external` check

The resolved ID (e.g. `'/project/node_modules/vue/dist/vue.runtime.esm-bundler.js'`) is tested against the [`external`](/reference/InputOptions.external) option with `isResolved: true`. If it matches, the specifier is marked as external.

### 5. Output path determination

Regardless of which step marked the module as external (first check, plugin, or second check), [`makeAbsoluteExternalsRelative`](/reference/InputOptions.makeAbsoluteExternalsRelative) applies uniformly to determine the import path in the output:

- **Bare specifiers** (e.g. `'lodash'`, `'node:fs'`) — appear as-is when matched on the first check. If matched on the second check (resolved path), the full resolved path appears instead (see the [caveat about `/node_modules/`](/reference/InputOptions.external#avoid-node-modules-for-npm-packages)).

- **Relative and absolute specifiers** — two things happen:
  1. **Resolve-time normalization** — for the first check and `return false`, when `makeAbsoluteExternalsRelative` is enabled (which it is by default), relative specifiers (the **original import specifier**) are normalized to absolute paths by resolving against the importer's directory. This ensures that `'./utils'` imported from different directories correctly maps to different external modules. For the second check and `return { id, external: true }`, the **resolved module ID** is already absolute.

  2. **Render-time output** — absolute resolved module IDs may be converted back to relative paths from the output chunk's location (e.g. `'/project/src/utils.js'` → `'./utils.js'`). Whether this happens depends on the `makeAbsoluteExternalsRelative` value and whether the original import specifier was relative.

Plugin overrides (`external: "relative"` / `"absolute"`) bypass this logic entirely. See the [`makeAbsoluteExternalsRelative` reference](/reference/InputOptions.makeAbsoluteExternalsRelative) for how each value controls this behavior, with examples.

## Special Cases

### Data URLs

Specifiers with a valid `data:` URL (e.g. `data:text/javascript,export default 42`) with a supported file format are handled by Rolldown's internal dataurl plugin which **bundles the inline content**. They are not automatically treated as external.

However, other `data:` URLs are treated as external automatically unless it's handled by a custom plugin.

### HTTP URLs

Specifiers starting with `http://`, `https://`, or `//` are **automatically treated as external** regardless of the `external` option, unless it's handled by a custom plugin. These IDs are emitted as-is and not affected by `makeAbsoluteExternalsRelative`.

```js
import lib from 'https://cdn.example.com/lib.js';
// Always external, emitted as-is
```
