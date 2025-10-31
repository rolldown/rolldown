# Bundling CJS

Rolldown provides first-class support for CommonJS modules. This document explains how Rolldown handles CJS modules and their interoperability with ES modules.

## Key Features

### Native CJS Support

Rolldown automatically recognizes and processes CommonJS modules without requiring any additional plugins or packages. This native support means:

- No need to install extra dependencies
- Better performance compared to plugin-based solutions

### On-demand Execution

Rolldown preserves the on-demand execution semantics of CommonJS modules, which is a key feature of the CommonJS module system. This means modules are only executed when they are actually required.

Here's an example:

```js
// index.js
import { value } from './foo.js';

const getFooExports = () => require('./foo.js');

// foo.js
module.exports = { value: 'foo' };
```

When bundled, it produces:

```js
// #region rolldown:runtime
// ...runtime code
// #endregion

// #region foo.js
var require_foo = __commonJS({
  'foo.js'(exports, module) {
    module.exports = { value: 'foo' };
  },
});

// #endregion
// #region index.js
const getFooExports = () => require_foo();
// #endregion
```

In this example, the `foo.js` module won't be executed until `getFooExports()` is called, maintaining the lazy-loading behavior of CommonJS.

### ESM/CJS Interoperability

Rolldown provides seamless interoperability between ES modules and CommonJS modules.

Example of ESM importing from CJS:

```js
// index.js
import { value } from './foo.js';

console.log(value);

// foo.js
module.exports = { value: 'foo' };
```

Bundled output:

```js
// #region rolldown:runtime
// ...runtime code
// #endregion

// #region foo.js
var require_foo = __commonJS({
  'foo.js'(exports, module) {
    module.exports = { value: 'foo' };
  },
});

// #endregion
// #region index.js
var import_foo = __toESM(require_foo());
console.log(import_foo.value);

// #endregion
```

The `__toESM` helper ensures that CommonJS exports are properly converted to ES module format, allowing seamless access to the exported values.

## `require` external modules

By default, Rolldown tries to keep the semantics of `require` and does not convert `require` against external modules to `import`. This is because the semantics of `require` are different from `import` in ES modules. For example, `require` are evaluated lazily, while `import` are evaluated before the code is executed.

::: tip Still want to convert `require` to `import`?

If you want to convert `require` calls to `import` statements, you can use [the built-in `esmExternalRequirePlugin`](/builtin-plugins/esm-external-require).

:::

For [`platform: 'node'`](../guide/notable-features.md#platform-presets), Rolldown will generate a `require` function from [`module.createRequire`](https://nodejs.org/docs/latest/api/module.html#modulecreaterequirefilename). This keeps the semantics of `require` completely intact. Note that compared to converting to `import`, there's two downsides to this approach:

1. Requires the `module.createRequire` function support in the runtime, which may not be available in partially Node compatible environments
2. Unsuitable for libraries that expects to be bundled as the `require` function will be a local variable and that makes it harder for bundlers to statically analyze the code

For other platforms, Rolldown will leave it as-is, allowing the running environment to provide a `require` function or inject one manually. For example, you can inject the `require` function that returns the value obtained by `import` by using [`inject` feature](../guide/notable-features.md#inject).

::: code-group

```js [rolldown.config.js]
import path from 'node:path';
export default {
  inject: {
    require: path.resolve('./require.js'),
  },
};
```

```js [require.js]
import fs from 'node:fs';

export default (id) => {
  if (id === 'node:fs') {
    return fs;
  }
  throw new Error(`Requiring ${JSON.stringify(id)} is not allowed.`);
};
```

:::

## Future Plans

Rolldown's first-class support for CommonJS modules enables several potential optimizations:

- Advanced tree-shaking capabilities for CommonJS modules
- Better dead code elimination
