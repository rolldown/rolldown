# About Rolldown

<!-- Note: this page is kept for potential external links to it, but no longer exposed in the navigation. -->

## TL;DR

Rolldown is a JavaScript bundler written in Rust intended to serve as the future bundler used in [Vite](https://vitejs.dev/). It provides Rollup-compatible APIs and plugin interface, but is more similar to esbuild in scope.

:::warning 🚧 Beta Software
Rolldown is currently in beta status. While it can already handle most production use cases, there may still be bugs and rough edges. Most notably, the built-in minification feature is still in early work-in-progress status.
:::

## Why we are building Rolldown

Rolldown is designed to serve as the future lower-level bundler used in [Vite](https://vitejs.dev/).

Currently, Vite relies on two bundlers internally:

- [esbuild](https://github.com/evanw/esbuild), created by [Evan Wallace](https://github.com/evanw). Vite uses esbuild for [Dependency Pre-Bundling](https://vitejs.dev/guide/dep-pre-bundling), TypeScript / JSX transforms, target lowering, and minification.

- [Rollup](https://github.com/rollup/rollup), created by [Rich Harris](https://github.com/Rich-Harris) and maintained by [Lukas Taegert-Atkinson](https://github.com/lukastaegert). Vite uses Rollup for its production builds, and supports a Rollup-compatible plugin interface.

Vite **has to** use two different bundlers because while both are amazing, they each lack something the other provides:

- esbuild is blazing fast and feature rich, but its output, especially in terms of chunk splitting limitations, is not ideal for bundling applications.

- Rollup is mature and battle tested for bundling applications, but is significantly slower than bundlers written in compile-to-native languages.

Having to use two different bundlers is suboptimal in several ways:

- Subtle differences between the output can cause behavior differences between development and production builds.

- User source is repeatedly parsed, transformed, and serialized by different tools throughout the production build, leading to a lot of overhead that can be avoided.

Ideally, we hope Vite can leverage a single bundler that provides native-level performance, built-in transforms that avoid parsing / serialization overhead, compatible plugin interface with Rollup, and advanced build output control that is suitable for large scale applications.

**This is why we are building Rolldown.**

Rolldown is written in [Rust](https://www.rust-lang.org/) and built on top of [Oxc](https://oxc-project.github.io/), leveraging its parser, resolver, transformer, and minifier (early WIP).

Our long term goal is for Vite users (directly or indirectly through a framework) to be able to transition to a Vite version that uses Rolldown internally with minimal friction.

At the same time, Rolldown can also be directly usable as a standalone bundler.

## Rollup compatibility & difference

Rolldown aims to align with Rollup's API and plugin interface as much as possible to ease adoption. In simple use cases, it will likely be able to serve as a drop-in replacement. However, it is also likely that there will be minor differences in edge cases, especially when advanced options are involved.

We started with the intention of a JS to Rust port, but soon realized that in order to achieve the best possible performance, we have to prioritize writing code in a way that aligns with how Rust works. The internal architecture of Rolldown is closer to that of esbuild rather than Rollup, and our chunk splitting logic may end up being different from that of Rollup's.

Rolldown's scope is also larger than Rollup's and more similar to esbuild. It comes with built-in ESM / CommonJS module interop, `node_modules` resolution, TypeScript / JSX transforms, and minification.

## Why Not Incrementally Improve Rollup?

Vite is standing on the shoulder of giants, and owes a lot of its success to Rollup. We are highly appreciative towards the brilliant work of Rollup's current maintainer [Lukas](https://github.com/lukastaegert). We reached out to Lukas before starting to work on Rolldown to make sure he is aware of and ok with it. The consensus was that it is good to explore both incremental improvements (by Lukas) and ground-up re-implementation (by us) in parallel.

Our thesis is that given the single-threaded nature of JavaScript and the complexity of bundlers, it is extremely unlikely to achieve the performance level we are aiming for via incremental changes. The performance gain from partially moving components to Rust is often significantly offset by the cost of passing data between Rust and JavaScript, as shown in Rollup 4's adoption of the Rust-based SWC parser. To achieve optimal performance, the entire parse / transform / codegen pipeline needs to happen on the native side, and be parallelized as much as possible. This is only feasible with a ground-up implementation, and is proven by Rolldown's 10~20x speed up compared to Rollup.

## Roadmap

Check out the [Roadmap](https://github.com/rolldown/rolldown/discussions/153) on GitHub discussions.

## Join Us!

Rolldown is still in early stage. We have a lot of ground to cover, and we won't be able to do this without the help from community contributors. We are also actively looking for more team members with long term commitment in improving JavaScript tooling with Rust.

### Useful Links

- [GitHub](https://github.com/rolldown/rolldown)
- [Contribution Guide](/contrib-guide/)
- [Discord Chat](https://chat.rolldown.rs)
