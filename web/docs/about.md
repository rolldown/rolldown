# About Rolldown

:::warning 🚧 Work in Progress
Rolldown is currently in active development and not usable for production yet.

Last updated: **March 8th, 2024**
:::

## The future bundler for Vite

It will provide Vite with:
- Significantly faster production builds
- Stronger dev / prod consistency
- More robust SSR dependency handling
- More control over chunk splitting

Our goal is for Vite users (directly or indirectly through a framework) to be able to transition to a Vite version that uses Rolldown internally with minimal friction.

At the same time, Rolldown will be usable as a standalone bundler

## Rollup Compatibility & Difference

- API & Plugin interface compatibility
- Internal logic difference
- Scope difference

## Roadmap

### Stage 1: Basic Bundling (done)
- Mixed CommonJS / ESM support

### Stage 2: Advanced Bundling (we are here)
- Treeshaking (done)
- Chunk hashing (wip)
- Source map (wip)
- Plugin compatibility (wip)
- Advanced Chunk splitting (planned)
- Module federation (planned)

### Stage 3: Built-in Transforms (work going on in parallel in oxc)
- TypeScript & JSX transforms
- Minification
- Syntax lowering

### Stage 4: Integration w/ Vite
- Plugin container w/ rustified Vite internal plugins
- Full bundle mode w/ HMR

### Nice to Haves

- Opinionated, zero config TypeScript library bundling
- DTS generation + bundling (isolatedDeclarations: true)

### Out of Scope

- CSS processing. Use Lightning CSS.
- Framework specific support (done via plugins)

## Acknowledgements

The Rolldown project is standing upon the shoulders of these giants:

- [rollup](https://github.com/rollup/rollup), created by [Rich-Harris](https://github.com/Rich-Harris) and maintained by [lukastaegert](https://github.com/lukastaegert).
- [esbuild](https://github.com/evanw/esbuild), created by [evanw](https://github.com/evanw).
- [parcel_sourcemap](https://github.com/parcel-bundler/source-map).

## Join us!

- [GitHub](https://github.com/rolldown-rs/rolldown)
- [Contribution Guide](/contrib-guide/)
- [Discord chat](https://discord.gg/vsZxvsfgC5)
