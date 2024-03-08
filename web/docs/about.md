<script setup>
import { VPTeamMembers } from 'vitepress/theme'

const members = [
  {
    avatar: 'https://www.github.com/yyx990803.png',
    name: 'Evan You',
    title: 'Project lead',
    links: [
      { icon: 'github', link: 'https://github.com/yyx990803' },
      { icon: 'twitter', link: 'https://twitter.com/youyuxi' }
    ]
  },
  {
    avatar: 'https://www.github.com/hyf0.png',
    name: 'Yunfei He',
    links: [
      { icon: 'github', link: 'https://github.com/hyf0' },
      { icon: 'twitter', link: 'https://twitter.com/_hyf0' }
    ]
  },
  {
    avatar: 'https://www.github.com/underfin.png',
    name: 'Kui Li (underfin)',
    links: [
      { icon: 'github', link: 'https://github.com/underfin' }
    ]
  }
]
</script>

# About Rolldown

## TL;DR

Rolldown is a JavaScript bundler written in Rust intended to serve as the future bundler used in Vite. It provides Rollup-compatible APIs and plugin interface, but will be more similar to esbuild in scope.


:::warning 🚧 Work in Progress
Rolldown is currently in active development and not usable for production yet. but we are open sourcing it now so we can start collaborating with community contributors.
:::

## Why we are building Rolldown

Rolldown is designed to serve as the future lower-level bundler used in [Vite](https://vitejs.dev/).

Currently, Vite relies on two bundlers internally:

- [esbuild](https://github.com/evanw/esbuild), created by [Evan Wallace](https://github.com/evanw). Vite uses esbuild for [Dependency Pre-Bundling](https://vitejs.dev/guide/dep-pre-bundling), TypeScript / JSX transforms, target lowering, and minification.

- [Rollup](https://github.com/rollup/rollup), created by [Rich Harris](https://github.com/Rich-Harris) and maintained by [Lukas Taegert-Atkinson](https://github.com/lukastaegert). Vite uses Rollup for its production builds, and supports a Rollup-compatible plugin interface.

Vite **has to** use two different bundlers because while both are amazing, they each lack something the other provides:

- esbuild is blazing fast and feature rich, but its output, especially in terms of chunk splitting limitations, is not ideal for bundling applications.

- Rollup is mature and battle tested for bundling applications, but is significantly slower than bundlers written in compile-to-native languages.

Having to use two different bundlers is sub-optimal in several ways:

- Subtle differences between the output can cause behavior differences between development and production builds.

- User source is repeatedly parsed, transformed, and serialized by different tools throughout the production build, leading to a lot of overhead that can be avoided.

Ideally, we hope Vite can leverage a single bundler that provides native-level performance, built-in transforms that avoid parsing / serialization overhead, compatible plugin interface with Rollup, and advanced build output control that is suitable for large scale applications.

**This is why we are building Rolldown.**

Rolldown is written in [Rust](https://www.rust-lang.org/) and built on top of [Oxc](https://oxc-project.github.io/), currently leveraging its parser and resolver. We also plan to leverage Oxc's transformer and minifier when they become available in the future.

Our long term goal is for Vite users (directly or indirectly through a framework) to be able to transition to a Vite version that uses Rolldown internally with minimal friction.

At the same time, Rolldown will also be directly usable as a standalone bundler.

## Rollup compatibility & difference

Rolldown aims to align with Rollup's API and plugin interface as much as possible to ease adoption. In simple use cases, it will likely be able to serve as a drop-in replacement. However, it is also likely that there will be minor differences in edge cases, especially when advanced options are involved.

We started with the intention of a JS to Rust port, but soon realized that in order to achieve the best possible performance, we have to prioritize writing code in a way that aligns with how Rust works. The internal architecture of Rolldown is closer to that of esbuild rather than Rollup, and our chunk splitting logic may end up being different from that of Rollup's.

Rolldown's scope is also larger than Rollup's and more similar to esbuild. It comes with built-in CommonJS support, `node_modules` resolution, and will also support TypeScript / JSX transforms and minification in the future.

## Roadmap

### Stage 1: Basic bundling (done)

- Mixed CommonJS / ESM support

### Stage 2: Advanced bundling (current stage)

- Treeshaking (done)
- Chunk hashing (WIP)
- Source map (WIP)
- Plugin compatibility (WIP)
- Advanced Chunk splitting (planned)
- Module federation (planned)

### Stage 3: Built-in transforms (WIP in parallel in Oxc)

- TypeScript & JSX transforms
- Minification
- Syntax lowering

### Stage 4: Integration w/ Vite

- Plugin container w/ rustified Vite internal plugins
- Full bundle mode w/ HMR

### Nice to haves

- Opinionated, zero config TypeScript library bundling
- DTS generation + bundling (requires [isolatedDeclarations](https://github.com/microsoft/TypeScript/issues/47947))

### Out of scope

- CSS processing. There is already a great Rust-based CSS build toolkit: [Lightning CSS](https://lightningcss.dev/).
- Framework specific support (these are expected to be done via plugins)

## Join Us!

Rolldown is still in early stage. We have a lot of ground to cover and we won't be able to do this without the help from community contributors. We are also actively looking for more team members with long term commitment in improving JavaScript tooling with Rust.

### Useful Links

- [GitHub](https://github.com/rolldown-rs/rolldown)
- [Contribution Guide](/contrib-guide/)
- [Discord Chat](https://discord.gg/vsZxvsfgC5)

### The Team

The Rolldown project is led by the creator of [Vite](https://vitejs.dev/), with team members who previously contributed to Vite and worked on [Rspack](https://www.rspack.dev/):

<VPTeamMembers size="small" :members="members" />
