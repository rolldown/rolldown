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
// #region \0rolldown/runtime.js
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
// #region \0rolldown/runtime.js
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

## Caveats

### `require` external modules

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

### Ambiguous `default` import from CJS modules

In the ecosystem, there's two common ways to handle imports from CJS modules. While Rolldown tries to support both interpretations automatically, they are **incompatible for `default` imports**. In that case, Rolldown uses a similar heuristic to [Webpack](https://webpack.js.org/) and [esbuild](https://esbuild.github.io/) to determine the value of `default` imports.

If it matches one of the conditions below, the `default` import is the `module.exports` value of the importee CJS module. Otherwise, the `default` import is the `module.exports.default` value of the importee CJS module.

- The importer is `.mjs` or `.mts`
- (When it's a dynamic import) The importer is `.cjs` or `.cts`
- The closest `package.json` for the importer has a `type` field set to `module`
- (When it's a dynamic import) The closest `package.json` for the importer has a `type` field set to `commonjs`
- The `module.exports.__esModule` value of the importee CJS module is not set to `true`

:::: details Behavior in details

Let's assume the following ESM importer module and CJS importee module:

::: code-group

```js [index.js]
import foo from './importee.cjs';
console.log(foo);
```

```js [importee.cjs]
Object.defineProperty(module.exports, '__esModule', {
  value: true,
});
module.exports.default = 'foo';
```

:::

In the first interpretation, the way [Babel](https://babel.dev/) interprets, this code will print `foo`. In this interpretation, the behavior is changed based on the `__esModule` flag. `__esModule` is commonly set by transformers to indicate that the module was written in ESM syntax (e.g. `export default 'foo'` in this case) and was transformed to CJS syntax. The rationale for this behavior is that the transformed module should behave the same as the original module did without the transformation. [`@rollup/plugin-commonjs`](https://github.com/rollup/plugins/tree/master/packages/commonjs) uses this interpretation by default.

In the second interpretation, the way Node.js interprets, this code will print `{ default: 'foo' }`. The rationale for this behavior is that CJS modules sets the export keys dynamically while ESM requires the export keys to be statically known, so to allow accessing all the exports, the entire `module.exports` is exposed as the default export. `@rollup/plugin-commonjs` uses this interpretation when `defaultIsModuleExports: false` is set.

These two interpretations expects different values for `default` imports and Rolldown has to decide which one to use.

::::

::: details What is the rationale for this heuristic?

Rolldown's heuristic is based on the assumption that the files affected by Node.js's module determination concept are expected to be runnable in Node.js. For ESM files to be runnable in Node.js, they need to have `.mjs` or the closest `package.json` to have a `type` field set to `module` ([so that the ESM loader is used](https://nodejs.org/api/packages.html#determining-module-system)), and the code should be written in a way that expects the Node.js interpretation. On the otherhand, for files written in ESM syntax but not marked as ESM in the Node.js's module determination concept, the code is highly likely to be transformed by other tools, which commonly follows the Babel's interpretation.

:::

#### Recommendations for Library Authors

If you are writing a new code, we strongly recommend you to **publish your code as ESM syntax**. With [the `require(ESM)` feature](https://nodejs.org/api/modules.html#loading-ecmascript-modules-using-require) shipped in Node.js, there's no major blocker to do so.
If you still need to publish your code as CJS syntax, we strongly recommend to **avoid using the `default` export**.

When importing a default export from a CJS module, we recommend to write a code that handles both interpretations. For example, you can use the following code to handle both interpretations:

```js
import rawFoo from './importee.cjs';
const foo =
  typeof rawFoo === 'object' && rawFoo !== null && rawFoo.__esModule ? rawFoo.default : rawFoo;
console.log(foo);
```

This code will print `foo` in both interpretations. Note that TypeScript may show a type error when using this code; this is because [TypeScript does not support this behavior](https://github.com/microsoft/TypeScript/issues/54102), but it is safe to ignore the error.

#### Recommendations for Library Users

If you find an issue that seems to be caused by this incompatibility, try using [publint](https://publint.dev/) to check the package. It has [a rule that detects the incompatibility](https://publint.dev/rules#cjs_with_esmodule_default_export) (note that it only checks some of the files in the package, not all of them).

If the heuristic is not working for you, you can use the code in the section above that handles both interpretations. If the import is in a dependency, we recommend to raise an issue to the dependency. In the meantime, you can use [`patch-package`](https://github.com/ds300/patch-package) or [`pnpm patch`](https://pnpm.io/cli/patch) or alternatives as an escape hatch.

## Future Plans

Rolldown's first-class support for CommonJS modules enables several potential optimizations:

- Advanced tree-shaking capabilities for CommonJS modules
- Better dead code elimination
