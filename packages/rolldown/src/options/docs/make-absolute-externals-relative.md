Despite the name, this option controls two things:

1. **Resolve-time normalization** — whether relative specifiers (e.g. `'./utils'`) are normalized to absolute paths internally for deduplication. Without normalization, `'./utils'` imported from different directories may collapse into one external module because they share the same raw string.
2. **Render-time output** — whether a resolved module ID (the absolute path after resolution) gets converted to a relative path in the output. It does not affect bare specifiers (e.g. `'lodash'`) or IDs that are already relative.

Both behaviors depend on the **original import specifier** (what you wrote in source code, e.g. `'./utils'`) vs the **resolved module ID** (the absolute path after resolution, e.g. `'/project/src/utils.js'`). See the [External Modules guide](/in-depth/external-modules) for how this fits into the full resolution flow.

#### Values

##### `"ifRelativeSource"` (default)

Only convert the resolved absolute ID to a relative path if the **original import specifier** was relative.

```js
// Original: relative specifier → converted to relative in output
import './lib/utils.js'; // → import './lib/utils.js'

// Original: absolute specifier → kept absolute in output
import '/project/lib/utils.js'; // → import '/project/lib/utils.js'
```

The idea: if you wrote a relative import, you probably want a relative import in the output. If you wrote an absolute import, you probably meant it to stay absolute.

##### `true`

Always convert resolved absolute IDs to relative paths:

```js
// Both become relative in output
import './lib/utils.js'; // → import './lib/utils.js'
import '/project/lib/utils.js'; // → import '../lib/utils.js'
```

When converting an absolute path to a relative path, Rolldown does _not_ take the [`file`](/reference/OutputOptions.file) or [`dir`](/reference/OutputOptions.dir) options into account, because those may not be present e.g. for builds using the JavaScript API. Instead, it assumes that the root of the generated bundle is located at the common shared parent directory of all entry points.

If the output chunk is itself nested in a subdirectory by choosing e.g. `chunkFileNames: "chunks/[name].js"`, the relative path is adjusted accordingly.

##### `false`

Never convert. Resolved absolute IDs are kept as-is. Relative specifiers are also **not** normalized to absolute paths internally, which means two files importing `'./utils'` from different directories may be treated as the same external module.

```js
import './lib/utils.js'; // → import './lib/utils.js' (as-is)
import '/project/lib/utils.js'; // → import '/project/lib/utils.js' (as-is)
```

::: warning Deduplication issue with `false`
Setting `makeAbsoluteExternalsRelative: false` disables the normalization of relative specifiers. This means `'./utils'` imported from `src/a.js` and `'./utils'` imported from `src/b/c.js` may be treated as the same external module, even though they refer to different files. Use `false` only if you are certain all your external specifiers are already unique (e.g. bare package names).
:::

#### Example

Given `import '/project/lib/utils.js'` (absolute specifier) in an external module, with output at `dist/index.js`:

| `makeAbsoluteExternalsRelative` | Output path               |
| ------------------------------- | ------------------------- |
| `true`                          | `'../lib/utils.js'`       |
| `"ifRelativeSource"` (default)  | `'/project/lib/utils.js'` |
| `false`                         | `'/project/lib/utils.js'` |

Given `import './lib/utils.js'` (relative specifier) with a flat output at `dist/index.js`:

| `makeAbsoluteExternalsRelative` | Output path        |
| ------------------------------- | ------------------ |
| `true`                          | `'./lib/utils.js'` |
| `"ifRelativeSource"` (default)  | `'./lib/utils.js'` |
| `false`                         | `'./lib/utils.js'` |

The same relative specifier with a nested chunk at `dist/chunks/index.js`:

| `makeAbsoluteExternalsRelative` | Output path         |
| ------------------------------- | ------------------- |
| `true`                          | `'../lib/utils.js'` |
| `"ifRelativeSource"` (default)  | `'../lib/utils.js'` |
| `false`                         | `'./lib/utils.js'`  |

With `true` or `"ifRelativeSource"`, relative specifiers are normalized to absolute paths internally, then re-relativized from the output chunk's location — so the path adjusts correctly for nested chunks. With `false`, the raw specifier is kept as-is with no adjustment.
