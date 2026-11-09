# ESM External Require Plugin

The `esmExternalRequirePlugin` is a built-in Rolldown plugin that converts CommonJS `require()` calls for external dependencies into ESM `import` statements, ensuring compatibility in environments that don't support the Node.js module API.

:::tip NOTE
This plugin sets `resolveId.meta.order` to `'pre'` to ensure external requires are resolved before other plugins. Additionally, it sets `enforce: 'pre'` by default for Vite compatibility.
:::

## Why This Is Needed

When bundling code with Rolldown, `require()` calls for external dependencies are not automatically converted to ESM imports to preserve the semantics of `require()`. While Rolldown injects `require` function when `platform: 'node'` is set, it does so by generating code like:

```js
import { createRequire } from 'node:module';
var __require = createRequire(import.meta.url);
```

However, this approach relies on the Node.js module API, which isn't available in some environments. This approach is also problematic for libraries that are expected to be bundled later, as this code is difficult to be analyzed and transformed by bundlers.

## Usage

Import and use the plugin from Rolldown's experimental exports:

```js
import { defineConfig } from 'rolldown';
import { esmExternalRequirePlugin } from 'rolldown/plugins';

export default defineConfig({
  input: 'src/index.js',
  output: {
    dir: 'dist',
    format: 'esm',
  },
  plugins: [
    esmExternalRequirePlugin({
      external: ['react', 'vue', /^node:/],
    }),
  ],
});
```

:::warning The plugin must own its externals
List each module in this plugin's `external` option or in the top-level `external` option, never both. Top-level `external` wins during resolution, so the plugin skips duplicated modules entirely. The build succeeds with a warning while the output keeps calling `require()` on the external module at runtime.
:::

## Options

### `external`

Type: `(string | RegExp)[]`

Defines which dependencies should be treated as external. When the output format is ESM, their `require()` calls will be converted to `import` statements. For non-ESM output formats, the dependencies will be marked as external but the `require()` calls will remain unchanged.

### `skipDuplicateCheck`

Type: `boolean`
Default: `false`

When enabled, skips checking for duplicate externals between this plugin and the top-level `external` option. This can improve build performance when you're confident there are no duplicates.

```javascript
esmExternalRequirePlugin({
  external: ['react', 'vue'],
  skipDuplicateCheck: true, // Skip duplicate check for better performance
});
```

## Duplicate External Detection

By default, the plugin checks if any externals you specify are also configured in the top-level `external` option. If duplicates are found, you'll see a warning:

```
Found 2 duplicate external: `react`, `vue`. Remove them from top-level `external` as they're already handled by 'builtin:esm-external-require' plugin.
```

Treat this warning as a correctness signal. The plugin leaves duplicated modules untouched: the top-level `external` option takes priority, so the output still contains the raw `require()` calls this plugin is meant to convert. Remove the duplicates from top-level `external`. Nothing is lost: the plugin marks its own modules as external anyway.

`skipDuplicateCheck: true` doesn't make duplicates work. It only silences the warning, so enable it only when you're certain no module appears in both places.

## Limitations

Since this plugin changes `require()` calls to `import` statements, there are some semantic differences after bundling:

- resolution is now based on `import` behavior, not `require` behavior
  - For example, `import` condition is used instead of `require` condition
- The values may be different from the original `require()` calls, especially for modules with default exports that don't expose a `'module.exports'` named export.

## How It Works

This plugin intercepts `require()` calls for dependencies specified in the option and creates virtual facade modules that:

1. Import the dependency using ESM `import * as m from '...'`
2. Use the dependency's `'module.exports'` named export when present, or fall back to a copy of its namespace
3. Replace the original `require()` with the virtual module reference

For non-external `require()` calls, Rolldown automatically wraps them and converts them into ESM imports.

```js
// Input code
const react = require('react');

// Transformed output
const react = require('builtin:esm-external-require-react');

// Virtual module: builtin:esm-external-require-react
import * as m from 'react';
module.exports = Object.prototype.hasOwnProperty.call(m, 'module.exports')
  ? m['module.exports']
  : { ...m };
```

The `'module.exports'` named export follows [Node.js CommonJS namespace semantics](https://nodejs.org/api/esm.html#commonjs-namespaces). Node.js v23.0.0 and later adds it to the namespace of every CommonJS module, so `require()` receives the exact `module.exports` value, including callable, `null`, and `undefined` values. Modules that don't expose this export fall back to a plain copy of the namespace. Node.js built-in modules use their default export directly.
