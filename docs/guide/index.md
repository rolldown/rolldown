# Introduction

## What is a Bundler

In JavaScript development, a bundler is responsible for compiling small pieces of code (ESM or CommonJS modules) into something larger and more complex, such as a library or application.

For web applications, this makes your application load and run significantly faster (even with HTTP/2). For libraries, this can avoid your consuming application having to bundle the source again, and can also improve runtime execution performance.

For those interested in the details, we have written a deeper analysis on [why bundlers are still needed](/guide/in-depth/why-bundlers).

## Why Rolldown

Rolldown is primarily designed to serve as the underlying bundler in [Vite](https://vite.dev/), with the goal to replace [esbuild](https://esbuild.github.io/) and [Rollup](https://rollupjs.org/) (which are currently used in Vite as dependencies) with one unified build tool. Here's why we are implementing a new bundler from the ground up:

- **Performance**: Rolldown is written in Rust. It is on the same performance level with esbuild and [10~30 times faster than Rollup](https://github.com/rolldown/benchmarks). Its WASM build is also [significantly faster than esbuild's](https://x.com/youyuxi/status/1869608132386922720) (due to Go's sub-optimal WASM compilation).

- **Ecosystem Compatibility**: Rolldown supports the same plugin API with Rollup / Vite, ensuring compatibility with Vite's existing ecosystem.

- **Additional Features**: Rolldown provides some important features needed in Vite but unlikely to be implemented by esbuild and Rollup (details below).

Although designed for Vite, Rolldown is also fully capable of being used as a standalone, general-purpose bundler. It can serve as a drop-in replacement for Rollup in most cases, and can also be used as an esbuild alternative when better chunking control is needed.

## Rolldown's Feature Scope

Rolldown provides largely compatible APIs (especially the plugin interface) with Rollup, and has similar treeshaking capabilities for bundle size optimization.

However, Rolldown's feature scope is more similar to esbuild, offering these [additional features](./features.md) as built-in:

- Platform presets
- TypeScript / JSX / syntax lowering transforms
- Node.js compatible module resolution
- ESM / CJS module interop
- `define`
- `inject`
- CSS bundling (Experimental)
- Minification (WIP)

Rolldown also has a few concepts that have close equivalents in esbuild, but do not exist in Rollup:

- [Module Types](./features#module-types) (Experimental)
- [Plugin hook filters](/plugins/hook-filters.md)

Finally, Rolldown provides some features that esbuild and Rollup do not (and may not intend to) implement:

- [Advanced chunk splitting control](./features#advanced-chunks) (Experimental)
- HMR support (WIP)
- Module Federation (planned)

## Credits

Rolldown wouldn't exist without all the lessons we learned from other bundlers like [esbuild](https://esbuild.github.io/), [Rollup](https://rollupjs.org/), [webpack](https://webpack.js.org/), and [Parcel](https://parceljs.org/). We have the utmost respect and appreciation towards the authors and maintainers of these important projects.
