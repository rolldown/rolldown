---
title: Announcing Rolldown 1.0 RC
author:
  - name: The Rolldown Team
sidebar: false
date: 2026-01-21
head:
  - - meta
    - property: og:type
      content: website
  - - meta
    - property: og:title
      content: Announcing Rolldown 1.0 RC
  - - meta
    - property: og:url
      content: https://rolldown.rs/blog/announcing-rolldown1-rc
  - - meta
    - property: og:description
      content: Rolldown 1.0 RC Release Announcement
---

# Announcing Rolldown 1.0 RC

_January 21, 2026_

Today we are thrilled to announce the Release Candidate for Rolldown 1.0.

**TL;DR:** Rolldown is a JavaScript/TypeScript bundler written in Rust. It is 10-30x faster than Rollup while maintaining compatibility with Rollup's plugin API. This RC marks API stability—no breaking changes are planned before 1.0.

## What is Rolldown?

Rolldown is a high-performance JavaScript bundler designed to serve as the future bundler for [Vite](https://vite.dev/). It combines the best of both worlds: the speed of [esbuild](https://esbuild.github.io/) and the ecosystem compatibility of [Rollup](https://rollupjs.org/). It also goes beyond both with features like [`output.codeSplitting`](/reference/OutputOptions.codeSplitting), which provides webpack-like granular chunking control.

## Try It Out

Getting started with Rolldown takes seconds:

::: code-group

```sh [npm]
$ npm install -D rolldown
```

```sh [pnpm]
$ pnpm add -D rolldown
```

```sh [yarn]
$ yarn add -D rolldown
```

```sh [bun]
$ bun add -D rolldown
```

:::

Create a config file:

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';

export default defineConfig({
  input: 'src/main.js',
  output: {
    file: 'dist/bundle.js',
  },
});
```

Run the build:

::: code-group

```sh [npm]
$ npx rolldown -c
```

```sh [pnpm]
$ pnpm rolldown -c
```

```sh [yarn]
$ yarn rolldown -c
```

```sh [bun]
$ bunx rolldown -c
```

:::

## Key Features

- **10-30x faster than Rollup** — Native Rust performance, with a WASM build also available
- **Rollup-compatible plugin API** — Existing Rollup plugins work out of the box
- **Built-in transforms** — TypeScript, JSX, and syntax lowering powered by [Oxc](https://oxc.rs/)
- **Native CJS/ESM interop** — No `@rollup/plugin-commonjs` needed
- **Built-in Node.js module resolution** — No `@rollup/plugin-node-resolve` needed
- **Manual code splitting** — Fine-grained chunking via `output.codeSplitting`

See the full list in [Notable Features](/guide/notable-features).

## What "RC" Means

This Release Candidate signals "API stability". We do not plan any breaking changes to Rolldown's public API before the 1.0 stable release.

**A note on experimental features:** Some features are still marked as experimental. For example:

- [Module types](/in-depth/module-types)
- [Watch mode](/reference/Function.watch)

Experimental features are clearly documented and may see breaking changes even after 1.0. We recommend testing experimental features thoroughly before using them in production.

## What We've Been Working On

Since beta.1, we have landed over 3,400 commits: 749 feature related commits, 682 bug fixes, 109 performance optimizations, and 166 documentation updates.

### Vite Integration

We ported multiple Vite's internal plugins to Rust, improving performance for common use cases. We have been testing it out in [rolldown-vite](https://v7.vite.dev/guide/rolldown) and the successor Vite 8 beta, improving the stability of Vite 8.

### Performance

109 performance commits including SIMD JSON escaping, parallel chunk generation, optimized symbol renaming, and faster sourcemap processing. Beyond Rolldown itself, [Oxc](https://oxc.rs/), which powers our transforms and resolver, also got faster.

### Better Chunking

We improved the chunking algorithm to produce fewer chunks. Dynamic imports that reference already-loaded modules are now inlined, and small wrapper chunks are merged with their target chunks when possible.

### Compatibility

We continue to expand Rollup and esbuild compatibility. Rolldown now passes 900+ Rollup tests and 670+ esbuild tests. Examples of newly added Rollup options:

- [`output.dynamicImportInCjs`](/reference/OutputOptions.dynamicImportInCjs) (control how dynamic imports are rendered in CJS output)
- [`watch.onInvalidate`](/reference/InputOptions.watch#oninvalidate) (hook for when a watched file triggers a rebuild)
- [`output.minifyInternalExports`](/reference/OutputOptions.minifyInternalExports) (minify internal export names for smaller bundles)

We also added support for Node.js `module.exports` ESM export, aligning with [Node.js's new behavior with `require(ESM)`](https://nodejs.org/docs/latest-v24.x/api/modules.html#loading-ecmascript-modules-using-require).

### API Stabilization

We promoted APIs from experimental and aligned defaults. For example:

- [`output.strictExecutionOrder`](/reference/OutputOptions.strictExecutionOrder) (moved from `experimental`)
- [`output.codeSplitting`](/reference/OutputOptions.codeSplitting) (renamed from `output.advancedChunks`)
- [`tsconfig` auto-discovery](/reference/InputOptions.tsconfig) enabled by default
- [`preserveEntrySignatures`](/reference/InputOptions.preserveEntrySignatures) defaults to `'exports-only'`
- Plugin timing diagnostics via [`checks.pluginTimings`](/reference/InputOptions.checks#plugintimings)

### Developer Experience

Better errors with documentation links, custom panic hooks for crash reporting, and new diagnostics aligned with Rollup like `CIRCULAR_REEXPORT` and `CANNOT_CALL_NAMESPACE`.

### Documentation

We filled in the missing docs:

- Dedicated pages for all options and functions and types in the [API reference](/reference/)
- [Plugin API](/apis/plugin-api) based on Rollup's documentation with Rolldown specific additions
- [CLI reference](/apis/cli)

## Roadmap to 1.0 and Vite 8

Our path forward:

1. **RC period**: Gather feedback, fix bugs, stabilize
2. **Vite 8**: Ships with Rolldown as the default bundler, replacing both esbuild and Rollup
3. **Rolldown 1.0**: Stable release with production-ready core features

The integration into Vite will unify the build pipeline, eliminating the current two-bundler architecture and providing consistent behavior between development and production.

## Acknowledgements

Rolldown 1.0 RC represents the collective effort of [over 150 contributors](https://github.com/rolldown/rolldown/graphs/contributors). Thank you to everyone who has contributed code, reported issues, or helped spread the word.

We also owe a great debt to the projects that inspired Rolldown:

- [Rollup](https://github.com/rollup/rollup) by Rich Harris and Lukas Taegert-Atkinson
- [esbuild](https://github.com/evanw/esbuild) by Evan Wallace

## Join the Community

- [Discord](https://chat.rolldown.rs) — Chat with the team and other users
- [GitHub](https://github.com/rolldown/rolldown) — Star the repo, report issues, contribute
- [Contributing Guide](https://rolldown.rs/contribution-guide/) — Get started contributing

## Give It a Try

Rolldown is ready for real-world testing. Try it on your projects and let us know how it goes. If you encounter issues, please [open an issue](https://github.com/rolldown/rolldown/issues) on GitHub. Your feedback during this RC period directly shapes the 1.0 release.

We cannot wait to see what you build with Rolldown.
