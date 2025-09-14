# ESM External Require Plugin

The `esmExternalRequirePlugin` is a built-in Rolldown plugin that converts CommonJS `require()` calls for external dependencies into ESM `import` statements, ensuring compatibility in environments that don't support the Node.js module API.

## Why This Is Needed

When bundling code with Rolldown, `require()` calls for external dependencies are not automatically converted to ESM imports to preserve the semantics of `require()`. While Rolldown injects `require` function when `platform: 'node'` is set, it does so by generating code like:

```JavaScript
import { createRequire } from 'node:module';
var __require = createRequire(import.meta.url);
```

However, this approach relies on the Node.js module API, which isn't available in some environments. This approach is also problematic for libraries that are expected to be bundled later, as this code is difficult to be analyzed and transformed by bundlers.

## Usage

Import and use the plugin from Rolldown's experimental exports:

```JavaScript
import { defineConfig } from 'rolldown';
import { esmExternalRequirePlugin } from 'rolldown/experimental';

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

## Options

### `external`

Type: `(string | RegExp)[]`

Defines which dependencies should be treated as external. When the output format is ESM, their `require()` calls will be converted to `import` statements. For non-ESM output formats, the dependencies will be marked as external but the `require()` calls will remain unchanged.

## Limitations

Since this plugin changes `require()` calls to `import` statements, there are some semantic differences after bundling:

- resolution is now based on `import` behavior, not `require` behavior
  - For example, `import` condition is used instead of `require` condition
- The values may be different from the original `require()` calls, especially for modules with default exports.

## How It Works

This plugin intercepts `require()` calls for dependencies specified in the option and creates virtual facade modules that:

1. Import the dependency using ESM `import * as m from '...'`
2. Re-export it using `module.exports = m` for CommonJS compatibility
3. Replace the original `require()` with the virtual module reference

For non-external `require()` calls, Rolldown automatically wraps them and converts them into ESM imports.

```JavaScript
// Input code
const react = require('react');

// Transformed output
const react = require('builtin:esm-external-require-react');

// Virtual module: builtin:esm-external-require-react
import * as m from 'react';
module.exports = m;
```
