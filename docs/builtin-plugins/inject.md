# Inject Plugin

The `injectPlugin` is a built-in Rolldown plugin that adds an import for each global variable a module uses. It does what [`transform.inject`](/reference/InputOptions.transform#inject) does, limited to the modules you pick with `include` and `exclude`. This is an equivalent of [`@rollup/plugin-inject`](https://github.com/rollup/plugins/tree/master/packages/inject).

## Usage

Import and use the plugin from Rolldown's plugins exports:

```js
import { defineConfig } from 'rolldown';
import { injectPlugin } from 'rolldown/plugins';

export default defineConfig({
  input: 'src/index.js',
  plugins: [
    injectPlugin(
      {
        // import { useState } from '#app'
        useState: ['#app', 'useState'],
      },
      {
        exclude: /node_modules/,
      },
    ),
  ],
});
```

The first argument takes the same patterns as [`transform.inject`](/reference/InputOptions.transform#inject):

```js
import path from 'node:path';

injectPlugin({
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
});
```

A module gets an import only for the names it uses as values and does not declare or import itself. Modules whose id contains a null byte (`\0`), such as virtual modules and Rolldown's runtime, are always skipped, like Rollup's `createFilter`.

## Options

### `include`

- **Type:** `string | RegExp | (string | RegExp)[]`

Only inject into modules whose id matches one of these patterns. Strings are glob patterns. Relative ones are resolved against the [`cwd`](/reference/InputOptions.cwd) option, or against `process.cwd()` when the plugin is called outside a Rolldown build, for example by Vite.

### `exclude`

- **Type:** `string | RegExp | (string | RegExp)[]`

Never inject into modules whose id matches one of these patterns, even if they match `include`.

## Different values per module

Each `injectPlugin` has its own values and filter, so you can inject a different value for the same name in different modules. Plugins run in order, and a module that already imports a name keeps that import.

```js
export default defineConfig({
  plugins: [
    // app code gets the framework's `foo`
    injectPlugin({ foo: ['framework', 'foo'] }, { exclude: /node_modules/ }),
    // other modules get the shim
    injectPlugin({ foo: ['foo-shim', 'foo'] }),
  ],
});
```

## Differences from `transform.inject`

`injectPlugin` skips `.cjs` modules, since Rolldown parses them as CommonJS and an `import` statement isn't valid there. Use `transform.inject` if those modules need the names.

`injectPlugin` sees the code before JSX and TypeScript are lowered:

- With the classic JSX runtime, injecting `React` has no effect, since `<div />` is not yet `React.createElement(...)`.
- In a TS module, an ambient declaration such as `declare const useKey: () => string` counts as a declaration, so `useKey` is not injected. `transform.inject` runs after it is stripped, so it injects `useKey`.

`injectPlugin` runs in the `transform` hook, so it sees the code before Rolldown applies [`transform.define`](/reference/InputOptions.transform#define). If you inject an object that a `define` key starts with, such as `process` and `process.env.NODE_ENV`, the define no longer matches, since `process` is no longer a global. Use [`replacePlugin`](/builtin-plugins/replace) for those values, or put the plugin that replaces them before `injectPlugin`.
