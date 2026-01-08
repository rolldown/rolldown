# Plugin API

:::warning 🚧 Under Construction
We are working on creating a more detailed reference. For now, please refer to [Rollup's Plugin API](https://rollupjs.org/plugin-development/) and [Plugin API Reference](/reference/Interface.Plugin.md).
:::

Rolldown's plugin interface is almost fully compatible with Rollup's (detailed tracking [here](https://github.com/rolldown/rolldown/issues/819)), so if you have written a Rollup plugin before, you already know how to write a Rolldown plugin!

We are still working on creating a more detailed guide for users who are new to both Rollup and Rolldown. For now, please first refer to [Rollup's plugin development guide](https://rollupjs.org/plugin-development/).

## Notable Differences from Rollup

While Rolldown's plugin interface is largely compatible with Rollup's, there are some important behavioral differences to be aware of:

### Output Generation Handling

In Rollup, all outputs are generated together in a single process. However, Rolldown handles each output generation separately. This means that if you have multiple output configurations, Rolldown will process each output independently, which can affect how certain plugins behave, especially those that maintain state across the entire build process.

These are the concrete differences:

- `outputOptions` hook is called **before** the build hooks in Rolldown, whereas Rollup calls them **after** the build hooks
- Build hooks are called for each output separately, whereas Rollup calls them once for all outputs
- `closeBundle` hook is called **only** when you called `generate()` or `write()` at least once, whereas Rollup calls it regardless of whether you called `generate()` or `write()`

### Sequential Hook Execution

In Rollup, certain hooks like [`writeBundle`](https://rollupjs.org/plugin-development/#writebundle) are "parallel" by default, meaning they run concurrently across multiple plugins. This requires plugins to explicitly set `sequential: true` if they need their hooks to run one after another.

In Rolldown, the `writeBundle` hook is already sequential by default, so plugins do not need to specify `sequential: true` for this hook.

## Builtin Plugins

Rolldown offers a set of built-in plugins, implemented in Rust, to achieve higher performance.

## Build Hooks

```hooks-graph
# styles
sequential: fillcolor="#ffe8cc", dark$fillcolor="#9d4f1a"
parallel: fillcolor="#ffcccc", dark$fillcolor="#8a2a2a"
first: fillcolor="#fff4cc", dark$fillcolor="#9d7a1a"
internal: fillcolor="#f0f0f0", dark$fillcolor="#3a3a3a"
sync: color="#3c3c43", dark$color="#dfdfd6"
async: color="#ff7e17", dark$color="#cc5f1a", penwidth=1

# nodes
watchChange(https://rollupjs.org/plugin-development/#watchchange): parallel, async
closeWatcher(https://rollupjs.org/plugin-development/#closewatcher): parallel, async
options(https://rollupjs.org/plugin-development/#options): sequential, async
outputOptions(https://rollupjs.org/plugin-development/#outputoptions): sequential, async
buildStart(https://rollupjs.org/plugin-development/#buildstart): parallel, async
resolveId(https://rollupjs.org/plugin-development/#resolveid): first, async
load(https://rollupjs.org/plugin-development/#load): first, async
transform(https://rollupjs.org/plugin-development/#transform): sequential, async
moduleParsed(https://rollupjs.org/plugin-development/#moduleparsed): parallel, async
internalTransform: internal
resolveDynamicImport(https://rollupjs.org/plugin-development/#resolvedynamicimport): first, async
buildEnd(https://rollupjs.org/plugin-development/#buildend): parallel, async

# edges
options -> outputOptions
outputOptions -> buildStart
buildStart -> resolveId: each entry
resolveId .-> buildEnd: external
resolveId -> load: non-external
load -> transform
transform -> internalTransform
internalTransform -> moduleParsed
moduleParsed .-> buildEnd: no imports
moduleParsed -> resolveDynamicImport: each import()
resolveDynamicImport -> load: non-external
moduleParsed -> resolveId: each import
resolveDynamicImport .-> buildEnd: external
resolveDynamicImport -> resolveId: unresolved
```

Note that `internalTransform` in the graph above is not a plugin hook, it is the step where Rolldown transforms non-JS code to JS.

::: warning Unsupported Hooks

The following Build Hooks are supported by Rollup, but not by Rolldown:

- `shouldTransformCachedModule` ([#4389](https://github.com/rolldown/rolldown/issues/4389))

:::

## Output Generation Hooks

```hooks-graph
# config
margin=150,0

# styles
sequential: fillcolor="#ffe8cc", dark$fillcolor="#9d4f1a"
parallel: fillcolor="#ffcccc", dark$fillcolor="#8a2a2a"
first: fillcolor="#fff4cc", dark$fillcolor="#9d7a1a"
internal: fillcolor="#f0f0f0", dark$fillcolor="#3a3a3a"
sync: color="#3c3c43", dark$color="#dfdfd6"
async: color="#ff7e17", dark$color="#cc5f1a", penwidth=1
!option: fillcolor="transparent"
!invisible: label="", shape=circle, fixedsize=true, width=0.2, height=0.2, style=filled, fillcolor="#ffffff"

# nodes
renderStart(https://rollupjs.org/plugin-development/#renderstart): parallel, sync
banner(https://rollupjs.org/plugin-development/#banner): sequential, sync
footer(https://rollupjs.org/plugin-development/#footer): sequential, sync
intro(https://rollupjs.org/plugin-development/#intro): sequential, sync
outro(https://rollupjs.org/plugin-development/#outro): sequential, sync
renderChunk(https://rollupjs.org/plugin-development/#renderchunk): sequential, sync
minify: internal
postBanner: option, sync
postFooter: option, sync
augmentChunkHash(https://rollupjs.org/plugin-development/#augmentchunkhash): sequential, async
generateBundle(https://rollupjs.org/plugin-development/#generatebundle): sequential, sync
writeBundle(https://rollupjs.org/plugin-development/#writebundle): parallel, sync
renderError(https://rollupjs.org/plugin-development/#rendererror): parallel, sync
closeBundle(https://rollupjs.org/plugin-development/#closebundle): parallel, sync
beforeAddons: invisible
afterAddons: invisible

# groups
generateChunks: beforeAddons, banner, footer, intro, outro, afterAddons

# edges
renderStart -> beforeAddons: each chunk
augmentChunkHash -> generateBundle
generateBundle -> writeBundle
writeBundle .-> closeBundle
beforeAddons -> banner
beforeAddons -> footer
beforeAddons -> intro
beforeAddons -> outro
banner -> afterAddons
footer -> afterAddons
intro -> afterAddons
outro -> afterAddons
afterAddons .-> beforeAddons: next chunk, constraint=false
afterAddons -> renderChunk: each chunk
renderChunk -> minify
minify -> postBanner
minify -> postFooter
postBanner -> augmentChunkHash
postFooter -> augmentChunkHash
augmentChunkHash .-> renderChunk: next chunk, constraint=false
renderError .-> closeBundle
```

Note that `minify` in the graph above is not a plugin hook and is the step where Rolldown runs the minifier. Also note that `postBanner` and `postFooter` are not plugin hooks, these are output options and do not have corresponding hooks, unlike `banner` and `footer`.

::: warning Unsupported Hooks

The following Output Generation Hooks are supported by Rollup, but not by Rolldown:

- `resolveImportMeta` ([#1010](https://github.com/rolldown/rolldown/issues/1010))
- `resolveFileUrl`
- `renderDynamicImport` ([#4532](https://github.com/rolldown/rolldown/issues/4532))

:::
