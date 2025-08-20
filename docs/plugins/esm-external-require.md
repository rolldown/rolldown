# ESM External Require Plugin

The `esmExternalRequirePlugin` is a built-in Rolldown plugin that converts CommonJS `require()` calls for external dependencies into ESM `import` statements, ensuring compatibility in environments that don't support the Node.js module API.

## Why This Is Needed

When bundling code with Rolldown, external dependencies that use `require()` calls are not automatically converted to ESM imports. Although setting `platform: 'node'` can convert `require` calls into `import`, it does so by generating code like:

```javascript
import { createRequire } from 'node:module';
var __require = createRequire(import.meta.url);
```

However, this approach relies on the Node.js module API, which isn't available in some environments.

## How It Works

The plugin intercepts `require()` calls for external dependencies and creates virtual facade modules that:

1. Import the dependency using ESM `import * as m from '...'`
2. Re-export it using `module.exports = m` for CommonJS compatibility
3. Replace the original `require()` with the virtual module reference

For non-external `require()` calls, Rolldown automatically wraps them and converts them into ESM imports.

```javascript
// Input code
const react = require('react');

// Transformed output
const react = require('builtin:esm-external-require-react');

// Virtual module: builtin:esm-external-require-react
import * as m from 'react';
module.exports = m;
```

## Usage

Import and use the plugin from Rolldown's experimental exports:

```javascript
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

Defines which dependencies should be treated as external and have their `require()` calls converted to imports.

## Limitations

The plugin only activates when:

- The output format is ESM
- The dependency is marked as external

**Note:** The plugin uses namespace imports (`import * as m`) which may have different interop behavior compared to direct CommonJS requires, especially for modules with default exports.
