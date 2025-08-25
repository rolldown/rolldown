# Introduction

Rolldown's plugin interface is almost fully compatible with Rollup's (detailed tracking [here](https://github.com/rolldown/rolldown/issues/819)), so if you have written a Rollup plugin before, you already know how to write a Rolldown plugin!

We are still working on creating a more detailed guide for users who are new to both Rollup and Rolldown. For now, please first refer to [Rollup's plugin development guide](https://rollupjs.org/plugin-development/).

## Notable Differences from Rollup

While Rolldown's plugin interface is largely compatible with Rollup's, there are some important behavioral differences to be aware of:

### Output Generation Handling

In Rollup, all outputs are generated together in a single process. However, Rolldown handles each output generation separately. This means that if you have multiple output configurations, Rolldown will process each output independently, which can affect how certain plugins behave, especially those that maintain state across the entire build process.

Related to that, the `outputOptions` hook is called **before** the build hooks in Rolldown, whereas Rollup calls them **after** the build hooks.

## Builtin Plugins

Rolldown offers a set of built-in plugins, implemented in Rust, to achieve higher performance.
