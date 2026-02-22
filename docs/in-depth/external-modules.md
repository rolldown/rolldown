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

### 1. The `external` Option

The [`external`](/reference/InputOptions.external) option in your config decides whether a module should be bundled or not.

```js
export default {
  external: ['lodash', /^node:/],
};
```

The `external` option is checked **twice** during resolution:

1. **First check** — the **raw import specifier** (e.g. `'./utils'`, `'lodash'`) before any resolution happens, with `isResolved: false`. If this matches, the specifier is normalized — Rolldown normalizes relative paths (see [Relative Specifiers](#relative-specifiers)) and picks the appropriate external variant. The **normalized ID** is used in the output.
2. **Second check** — the **resolved ID** (e.g. `'/project/node_modules/vue/dist/vue.runtime.esm-bundler.js'`) after plugins and the internal resolver have run, with `isResolved: true`. If this matches, the **already-resolved ID** is used directly as the module ID in the output — no normalization occurs.

If the first check matches, the module is immediately marked as external — plugins and the internal resolver are skipped entirely. The second check only runs if the first did not match.

::: warning Avoid `/node_modules/` as an external pattern
The pattern `external: /node_modules/` only matches on the second check (resolved absolute path), so the full resolved path like `/path/to/node_modules/vue/dist/vue.runtime.esm-bundler.js` ends up in the output verbatim. This makes the output non-portable.

Instead, match packages by name or use a bare-specifier pattern:

```js
export default {
  // Exact package names
  external: ['vue', 'react', 'react-dom'],

  // Package name patterns
  external: [/^vue/, /^react/, /^@mui/],

  // All bare module IDs (not starting with `.` or `/`)
  external: /^[^./]/,
};
```

:::

### 2. A Plugin's `resolveId` Hook

A plugin can explicitly mark a module as external by returning `external: true` (or `"relative"` / `"absolute"`) from the `resolveId` hook:

```js
{
  name: 'my-plugin',
  resolveId(source) {
    if (source === 'my-virtual-external') {
      return { id: source, external: true };
    }
  },
}
```

A plugin can also return `false` to mark a module as external. This sends the raw specifier through the same normalization as the `external` option's first check — relative specifiers are normalized to absolute paths (for deduplication), [`makeAbsoluteExternalsRelative`](#makeabsoluteexternalsrelative) is consulted, and the output path is re-relativized at render time:

```js
{
  name: 'my-plugin',
  resolveId(source) {
    if (source.startsWith('my-lib/')) {
      return false; // external, normalize like the `external` option
    }
  },
}
```

### 3. Unresolved Modules

If a module cannot be resolved (not found on disk, no plugin handles it) and the `external` option matches the specifier, Rolldown treats it as external rather than throwing an error.

## What Path Appears in the Output?

Once a module is external, the next question is: **what import path will appear in the output bundle?** This depends on the type of specifier and the `makeAbsoluteExternalsRelative` option.

### Bare Specifiers

Bare specifiers like `'lodash'` or `'node:fs'` appear as-is in the output when matched on the **first check** (raw specifier). Since they aren't relative or absolute paths, no normalization is applied.

```js
// input
import _ from 'lodash';
// output — always the same
import _ from 'lodash';
```

However, if a bare specifier is _not_ matched on the first check and instead goes through the resolver, the second check matches the **resolved absolute path** — and that resolved path is what appears in the output. See the [warning above](#_1-the-external-option) about `/node_modules/`.

### Relative Specifiers

When [`makeAbsoluteExternalsRelative`](#makeabsoluteexternalsrelative) is enabled (the default), relative specifiers like `'./utils.js'` are **normalized to absolute paths internally** via path concatenation (not full module resolution). The importer's directory is joined with the relative specifier and cleaned up. This ensures that two files importing `'./utils'` from different directories are correctly identified as **different** external modules.

In the output, the absolute path is re-relativized from the output chunk's location:

```js
// input: src/index.js
import { foo } from './lib/utils.js';

// output: dist/index.js
import { foo } from './lib/utils.js'; // relative to dist/
```

### Absolute Specifiers

Absolute specifiers like `'/project/lib/utils.js'` are where `makeAbsoluteExternalsRelative` comes into play.

## `makeAbsoluteExternalsRelative`

This option controls whether absolute external paths are converted to relative paths in the output. It accepts three values:

### `"ifRelativeSource"` (default)

Only convert to relative if the **original import specifier** was relative.

```js
// Original: relative specifier → converted to relative in output
import './lib/utils.js'; // → import './lib/utils.js' (relative to chunk)

// Original: absolute specifier → kept absolute in output
import '/project/lib/utils.js'; // → import '/project/lib/utils.js'
```

The idea: if you wrote a relative import, you probably want a relative import in the output. If you wrote an absolute import, you probably meant it to stay absolute.

When enabled (`"ifRelativeSource"` or `true`), relative specifiers are normalized to absolute paths internally for deduplication, then re-relativized at render time relative to the output chunk.

### `true`

Always convert absolute external paths to relative:

```js
// Both become relative in output
import './lib/utils.js'; // → import './lib/utils.js'
import '/project/lib/utils.js'; // → import '../lib/utils.js'
```

### `false`

Never convert. All paths are kept as-is. Relative specifiers are **not** normalized internally either, which means two files importing `'./utils'` from different directories may be treated as the same external module.

```js
import './lib/utils.js'; // → import './lib/utils.js' (as-is)
import '/project/lib/utils.js'; // → import '/project/lib/utils.js' (as-is)
```

::: warning Deduplication issue with `false`
Setting `makeAbsoluteExternalsRelative: false` disables the internal normalization of relative specifiers. This means `'./utils'` imported from `src/a.js` and `'./utils'` imported from `src/b/c.js` may be treated as the same external module, even though they refer to different files. Use `false` only if you are certain all your external specifiers are already unique (e.g. bare package names).
:::

### Summary Table

Given `import '/project/lib/utils.js'` (absolute specifier) in an external module, with output at `dist/index.js`:

| `makeAbsoluteExternalsRelative` | Output path               |
| ------------------------------- | ------------------------- |
| `true`                          | `'../lib/utils.js'`       |
| `"ifRelativeSource"` (default)  | `'/project/lib/utils.js'` |
| `false`                         | `'/project/lib/utils.js'` |

Given `import './lib/utils.js'` (relative specifier):

| `makeAbsoluteExternalsRelative` | Output path        |
| ------------------------------- | ------------------ |
| `true`                          | `'./lib/utils.js'` |
| `"ifRelativeSource"` (default)  | `'./lib/utils.js'` |
| `false`                         | `'./lib/utils.js'` |

The three settings only produce different results for **absolute specifiers**.

## Plugin Control: `external` Values in `resolveId`

When a plugin returns from `resolveId`, it can set `external` to different values to control the output path format:

### `external: true`

The module is external. Whether the path is relativized depends on `makeAbsoluteExternalsRelative` — the plugin **defers** to the user's config.

```js
resolveId(source) {
  return { id: '/project/lib/utils.js', external: true };
  // Output path depends on makeAbsoluteExternalsRelative
}
```

### `external: "relative"`

The module is external. The path is **always** relativized in the output, regardless of `makeAbsoluteExternalsRelative`. The plugin **overrides** the user's config.

```js
resolveId(source) {
  return { id: '/project/lib/utils.js', external: 'relative' };
  // Output: '../lib/utils.js' (always relative, ignores config)
}
```

Use this when your plugin resolves to an absolute path for deduplication but always wants a relative import in the output.

### `external: "absolute"`

The module is external. The path is **always** kept verbatim in the output, regardless of `makeAbsoluteExternalsRelative`. The plugin **overrides** the user's config.

```js
resolveId(source) {
  return { id: '/project/lib/utils.js', external: 'absolute' };
  // Output: '/project/lib/utils.js' (always absolute, ignores config)
}
```

Use this when your plugin intentionally wants an absolute or synthetic path in the output, such as server-side environments or virtual module IDs.

::: tip `"relative"` and `"absolute"` are plugin overrides
These values override `makeAbsoluteExternalsRelative` entirely. `"absolute"` really means "don't touch this ID" — it can be used to preserve _any_ ID verbatim, including relative paths. For example, `{ id: './utils', external: 'absolute' }` emits `'./utils'` exactly as-is in the output.
:::

### `return false`

Returning `false` from `resolveId` means "this module is external." Under the hood, the raw specifier is sent through the same normalization as the `external` option's first check: relative specifiers are normalized to absolute paths for deduplication, `makeAbsoluteExternalsRelative` is consulted, and the output path is re-relativized at render time.

```js
resolveId(source) {
  if (source.startsWith('my-lib/')) return false;
  // Equivalent to: the external option matching this specifier on the first check
}
```

### Summary

| Plugin `external` value | `makeAbsoluteExternalsRelative` consulted? | Who decides path format? |
| ----------------------- | ------------------------------------------ | ------------------------ |
| `true`                  | Yes                                        | User config              |
| `"relative"`            | No — always relativize                     | Plugin                   |
| `"absolute"`            | No — always keep verbatim                  | Plugin                   |
| `return false`          | Yes                                        | User config              |

## The Full Resolution Flow

### Step by step

1. **First `external` check** — test the raw specifier (`isResolved: false`). If it matches, the specifier is normalized and marked as external.

2. **Plugin `resolveId`** — plugins get a chance to resolve the import.
   - `return false` → normalizes the raw specifier and marks it external (same path as step 1).
   - `return { id, external: true | "relative" | "absolute" }` → the module is external with the given ID.
   - `return { id }` (no `external`) → continue with the resolved ID.
   - `return null` → no plugin handled it, fall through.

3. **Internal resolver** — Rolldown's built-in resolver tries to find the module on disk.

4. **Second `external` check** — test the resolved ID (`isResolved: true`). If it matches, the resolved ID is used **as-is** in the output. No normalization.

5. **Variant selection** — based on `makeAbsoluteExternalsRelative` and whether the original specifier was relative, the external module is tagged as `Relative` (re-relativize at render) or `Absolute` (keep verbatim). Plugin overrides (`"relative"` / `"absolute"`) bypass this step.

## Special Cases

### Data URLs and HTTP URLs

Specifiers starting with `data:`, `http://`, `https://`, or `//` are **automatically treated as external** before the internal resolver runs, regardless of the `external` option. These IDs are not affected by `makeAbsoluteExternalsRelative`.

```js
import data from 'data:text/javascript,export default 42';
import lib from 'https://cdn.example.com/lib.js';
// Both are always external, emitted as-is
```
