# Notable Features

This page documents some notable features in Rolldown that do not have built-in equivalents in Rollup.

## Platform presets

- Configurable via the `platform` option.
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

Rolldown supports the following transforms out of the box, powered by [Oxc](https://github.com/oxc-project/oxc):

- TypeScript
- JSX
  - Configurable via the `jsx` option, aligned with [Rollup's `jsx` option](https://rollupjs.org/configuration-options/#jsx)
- Syntax lowering transforms <sup>WIP</sup>
  - Landing soon, configurable via the `target` option

## ESM / CJS Interop

Rolldown handles mixed ESM / CJS module graphs out of the box, without the need for `@rollup/plugin-commonjs`. It largely follows esbuild's semantics and [passes all esbuild ESM / CJS interop tests](https://github.com/rolldown/bundler-esm-cjs-tests).

## Module resolution

- Powered by [oxc-resolver](https://github.com/oxc-project/oxc-resolver), aligned with webpack's [enhanced-resolve](https://github.com/webpack/enhanced-resolve)
- `node_modules` resolution is enabled by default (equivalent of `@rollup/plugin-node-resolve`)
- tsconfig paths supported via `resolve.tsconfigFilename`.
- Configurable via the `resolve` option:

  ```ts
  interface InputOptions {
    resolve?: {
      alias?: Record<string, string[] | string>;
      aliasFields?: string[][];
      conditionNames?: string[];
      extensionAlias?: Record<string, string[]>;
      exportsFields?: string[][];
      extensions?: string[];
      mainFields?: string[];
      mainFiles?: string[];
      modules?: string[];
      symlinks?: boolean;
      tsconfigFilename?: string;
    };
  }
  ```

  When `tsconfigFilename` is provided, the resolver will respect `compilerOptions.paths` in the specified `tsconfig.json`.

## Define

- Configurable via the `define` option.

This feature provides a way to replace global identifiers with constant expressions. Aligns with the respective options in [Vite](https://vite.dev/config/shared-options.html#define) and [esbuild](https://esbuild.github.io/api/#define).

Note it behaves differently from [`@rollup/plugin-replace`](https://github.com/rollup/plugins/tree/master/packages/replace) as the replacement is AST-based, so the value to be replaced must be a valid identifier or member expression.

## Inject

- Configurable via the `inject` option.

This is the feature equivalent of [esbuild's `inject` option](https://esbuild.github.io/api/#inject) and [`@rollup/plugin-inject`](https://github.com/rollup/plugins/tree/master/packages/inject).

The API is aligned with `@rollup/plugin-inject`:

```js [rolldown.config.js]
export default {
  inject: {
    // import { Promise } from 'es6-promise'
    Promise: ['es6-promise', 'Promise'],

    // import { Promise as P } from 'es6-promise'
    P: ['es6-promise', 'Promise'],

    // import $ from 'jquery'
    $: 'jquery',

    // import * as fs from 'node:fs'
    fs: ['node:fs', '*'],

    // Inject shims for property access pattern
    'Object.assign': path.resolve('src/helpers/object-assign.js'),
  },
};
```

## CSS bundling

- ⚠️ Experimental

Rolldown supports bundling CSS imported from JS out of the box. Note this feature currently does not support CSS Modules and minification.

## Advanced Chunks

- ⚠️ Experimental
- Similar to webpack's [`optimization.splitChunks`](https://webpack.js.org/plugins/split-chunks-plugin/#optimizationsplitchunks)
- Configurable via `output.advancedChunks`:

```ts
interface OutputOptions {
  advancedChunks?: {
    minSize?: number;
    minShareCount?: number;
    groups?: {
      name: string;
      test?: StringOrRegExp;
      priority?: number;
      minSize?: number;
      minShareCount?: number;
    }[];
  };
}
```

## Module types

- ⚠️ Experimental

This is conceptually similar to [esbuild's `loader` option](https://esbuild.github.io/api/#loader), allowing users to globally associate file extensions to built-in module types via the `moduleTypes` option, or specify module type of a specific module in plugin hooks. It is discussed in more details [here](/guide/in-depth/module-types).

## Minification

- ⚠️ WIP
- Configurable via the `output.minify` option.

This is powered by [`oxc-minifier`](https://github.com/oxc-project/oxc/tree/main/crates/oxc_minifier), which is currently still work-in-progress. There is no configurability yet and the compression quality is not production ready. Expect improvements in the future!

For now, it is recommended to use an external minifier for production use cases. Rolldown is compatible with Rollup minifier plugins:

With [`rollup-plugin-esbuild`](https://github.com/egoist/rollup-plugin-esbuild):

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';
import { minify } from 'rollup-plugin-esbuild';

export default defineConfig({
  plugins: [minify()],
});
```

With [`rollup-plugin-swc3`](https://github.com/SukkaW/rollup-plugin-swc):

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
