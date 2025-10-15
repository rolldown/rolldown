# Notable Features

This page documents some notable features in Rolldown that do not have built-in equivalents in Rollup.

## Platform presets

- Configurable via the [`platform`](/apis/config-options#platform) option.
- Default: `browser`
- Possible values: `browser | node | neutral`

Similar to [esbuild's `platform` option](https://esbuild.github.io/api/#platform), this option provides some sensible defaults regarding module resolution and how to handle `process.env.NODE_ENV`.

**Notable differences from esbuild:**

- The default output format is always `esm` regardless of platform.
- No `</script>` escape behavior when platform is `browser`.

:::tip
Rolldown does not polyfill Node built-ins when targeting the browser. You can opt-in to it with [rolldown-plugin-node-polyfills](https://github.com/rolldown/rolldown-plugin-node-polyfills).
:::

## Built-in transforms

Rolldown supports the following transforms out of the box, powered by [Oxc](https://oxc.rs/docs/guide/usage/transformer).
The transform is configurable via the [`transform`](/apis/config-options#transform) option.
The following transforms are supported:

- TypeScript
  - Sets configurations based on the `tsconfig.json` when [`tsconfig`](/apis/config-options#tsconfig) option is provided.
  - Supported legacy decorators and decorator metadata.
- JSX
- Syntax lowering
  - Automatically transforms modern syntax to be compatible with your defined [target](/apis/config-options#transform).
  - Supports [down to ES2015](https://oxc.rs/docs/guide/usage/transformer/lowering#transformations).

## CJS support

Rolldown supports mixed ESM / CJS module graphs out of the box, without the need for `@rollup/plugin-commonjs`. It largely follows esbuild's semantics and [passes all esbuild ESM / CJS interop tests](https://github.com/rolldown/bundler-esm-cjs-tests).

See [Bundling CJS](/in-depth/bundling-cjs) for more details.

## Module resolution

- Configurable via the [`resolve`](/apis/config-options#resolve) option
- Powered by [oxc-resolver](https://github.com/oxc-project/oxc-resolver), aligned with webpack's [enhanced-resolve](https://github.com/webpack/enhanced-resolve)

Rolldown resolves modules based on TypeScript and Node.js' behavior by default, without the need for `@rollup/plugin-node-resolve`.

When top-level [`tsconfig`](/apis/config-options#tsconfig) option is provided, Rolldown will respect `compilerOptions.paths` in the specified `tsconfig.json`.

## Define

- Configurable via the [`define`](/apis/config-options#define) option.

This feature provides a way to replace global identifiers with constant expressions. Aligns with the respective options in [Vite](https://vite.dev/config/shared-options.html#define) and [esbuild](https://esbuild.github.io/api/#define).

::: tip `@rollup/plugin-replace` behaves differently

Note it behaves differently from [`@rollup/plugin-replace`](https://github.com/rollup/plugins/tree/master/packages/replace) as the replacement is AST-based, so the value to be replaced must be a valid identifier or member expression. Use the built-in [`replacePlugin`](/builtin-plugins/replace) for that purpose.

:::

## Inject

- Configurable via the [`inject`](/apis/config-options#inject) option.

This feature provides a way to shim global variables with a specific value exported from a module. This feature is equivalent of [esbuild's `inject` option](https://esbuild.github.io/api/#inject) and [`@rollup/plugin-inject`](https://github.com/rollup/plugins/tree/master/packages/inject).

## CSS bundling

- ⚠️ Experimental

Rolldown supports bundling CSS imported from JS out of the box. Note this feature currently does not support CSS Modules and minification.

## Advanced Chunks

- ⚠️ Experimental
- Configurable via [`output.advancedChunks`](/apis/config-options#advancedchunks) option.

Rolldown allows controlling the chunking behavior granularly, similar to webpack's [`optimization.splitChunks`](https://webpack.js.org/plugins/split-chunks-plugin/#optimizationsplitchunks) feature.

See [Advanced Chunks](/in-depth/advanced-chunks) for more details.

## Module types

- ⚠️ Experimental

This is conceptually similar to [esbuild's `loader` option](https://esbuild.github.io/api/#loader), allowing users to globally associate file extensions to built-in module types via the `moduleTypes` option, or specify module type of a specific module in plugin hooks. It is discussed in more details [here](/in-depth/module-types).

## Minification

- ⚠️ Experimental
- Configurable via the [`output.minify`](/apis/config-options#minify) option.

The minification is powered by [`oxc-minifier`](https://github.com/oxc-project/oxc/tree/main/crates/oxc_minifier), which is currently in alpha and can still have bugs. We recommend thoroughly testing your output in production environments.

If you prefer an external minifier instead, Rolldown is compatible with Rollup minifier plugins, such as:

[`rollup-plugin-esbuild`](https://github.com/egoist/rollup-plugin-esbuild):

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';
import { minify } from 'rollup-plugin-esbuild';

export default defineConfig({
  plugins: [minify()],
});
```

[`rollup-plugin-swc3`](https://github.com/SukkaW/rollup-plugin-swc):

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';
import { minify } from 'rollup-plugin-swc3';

export default defineConfig({
  plugins: [
    minify({
      module: true,
      // swc's minify option here
      mangle: {},
      compress: {},
    }),
  ],
});
```
