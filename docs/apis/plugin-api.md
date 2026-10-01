# Plugin API

## Overview

Rolldown's plugin interface is almost fully compatible with Rollup's (detailed tracking [here](https://github.com/rolldown/rolldown/issues/819)), so if you have written a Rollup plugin before, you already know how to write a Rolldown plugin!

A Rolldown plugin is an object that satisfies the [plugin interface](#plugin-interface) described below.
A plugin should be distributed as a package which exports a function that can be called with plugin specific options and returns such an object.

Plugins allow you to customize Rolldown's behavior by, for example, transpiling code before bundling, or shimming a built-in module that is not available.

<!-- TODO: add a link to a guide on how to use plugins & how to find plugins -->

### Example

The following example shows a Rolldown plugin that intercepts import requests to `virtual:example` and returns a custom content for it.

```js displayName="rolldown-plugin-example.js"
const id = 'virtual:example';
const resolvedId = '\0' + id;

export default function examplePlugin() {
  return {
    name: 'example-plugin', // this name will show up in logs and errors
    resolveId(source) {
      if (source === id) {
        // this signals to Rolldown that this import should resolve to a module named `\0virtual:example`
        return resolvedId;
      }
      return null; // other ids should be handled as usual
    },
    load(id) {
      if (id === resolvedId) {
        // the source code for `\0virtual:example`
        return `export default 'Hello from ${id}';`;
      }
      return null; // other ids should be handled as usual
    },
  };
}
```

```js displayName="rolldown.config.js"
import { defineConfig } from 'rolldown';
import examplePlugin from './rolldown-plugin-example.js';

export default defineConfig({
  plugins: [examplePlugin()],
});
```

> [!WARNING]
> **Hook Filters**
>
> This example plugin does not use [Hook Filters](/apis/plugin-api/hook-filters) for simplicity.
> To improve performance, it is recommended to use them when possible.

## Conventions

- Plugins should have a clear name with `rolldown-plugin-` prefix.
- Include `rolldown-plugin` keyword in the package.json `keywords` field.
- Make sure your plugin outputs correct source mappings if appropriate.
- If your plugin uses [virtual modules](#virtual-modules), follow the [Virtual Modules Convention](#virtual-modules).
- (recommended) Plugins should be tested.
- (recommended) Plugins should be documented in English.

<!-- TODO: add a guide how to test a plugin -->

<div id="virtual-modules"></div>

### Virtual Modules Convention

Virtual modules are a useful scheme that allows you to pass build time information or helper functions to source files using normal ESM import syntax. A virtual module is a module that does not exist on the file system and is instead resolved and provided by a plugin, as shown in the [example above](#example).

Once such a plugin is registered, the virtual module can be imported in JavaScript through its user-facing id:

```js
import msg from 'virtual:example';

console.log(msg);
```

Virtual modules in Rolldown are prefixed with `virtual:` for the user-facing path by convention. If possible the plugin name should be used as a namespace to avoid collisions with other plugins in the ecosystem. For example, a `rolldown-plugin-posts` could ask users to import a `virtual:posts` or `virtual:posts/helpers` virtual module to get build time information. Internally, plugins that use virtual modules should prefix the module ID with `\0` while resolving the id, a convention from the Rollup ecosystem. This prevents other plugins from trying to process the id (like node resolution), and core features like sourcemaps can use this info to differentiate between virtual modules and regular files.

Note that modules directly derived from a real file, as in the case of a script module in a Single File Component (like a `.vue` or `.svelte` SFC), don't need to follow this convention. SFCs generally generate a set of submodules when processed, but the code in these can be mapped back to the filesystem. Using `\0` for these submodules would prevent sourcemaps from working correctly.

## Plugin Interface

The [`Plugin`](/reference/Interface.Plugin) interface has a required `name` property and multiple optional properties and hooks.

Hooks are methods defined on the plugin that can be used to interact with the build process. They are called at various stages of the build. Hooks can affect how a build is run, provide information about a build, or modify a build once complete. There are different kinds of hooks:

- `async`: The hook may also return a Promise resolving to the same type of value; otherwise, the hook is marked as `sync`.
- `first`: If several plugins implement this hook, the hooks are run sequentially until a hook returns a value other than `null` or `undefined`.
- `sequential`: If several plugins implement this hook, all of them will be run in the specified plugin order. If a hook is `async`, subsequent hooks of this kind will wait until the current hook is resolved.
- `parallel`: If several plugins implement this hook, all of them will be run in the specified plugin order. If a hook is `async`, subsequent hooks of this kind will be run in parallel and not wait for the current hook.

Instead of a method, hooks can also be objects with a `handler` property. In this case, the `handler` property is the actual hook method. This allows you to provide additional optional properties to control the behavior of the hook. See the [`ObjectHook`](/reference/TypeAlias.ObjectHook) type for more information.

There are two types of hooks: [build hooks](#build-hooks) and [output generation hooks](#output-generation-hooks).

### Build Hooks

Build hooks are run during the build phase. They are mainly concerned with locating, providing and transforming input files before they are processed by Rolldown.

The first hook of the build phase is [`options`](/reference/Interface.Plugin#options), the last one is always [`buildEnd`](/reference/Interface.Plugin#buildend). If there is a build error, [`closeBundle`](/reference/Interface.Plugin#closebundle) will be called after that.

```dot
digraph {
    bgcolor="transparent";
    rankdir=TB;
    node [shape=box, style=filled, fontname="Arial", margin="0.2,0.1", color="${#3c3c43|#dfdfd6}", fontcolor="${#3c3c43|#dfdfd6}"];
    edge [fontname="Arial", color="${#3c3c43|#dfdfd6}"];

    // Node definitions with styling
    watchchange [label="watchChange", fillcolor="${#ffcccc|#8a2a2a}", color="${#ff7e17|#cc5f1a}", penwidth=1, style="filled,rounded", href="/reference/Interface.Plugin#watchchange"];
    closewatcher [label="closeWatcher", fillcolor="${#ffcccc|#8a2a2a}", color="${#ff7e17|#cc5f1a}", penwidth=1, style="filled,rounded", href="/reference/Interface.Plugin#closewatcher"];
    options [label="options", fillcolor="${#ffe8cc|#9d4f1a}", color="${#ff7e17|#cc5f1a}", penwidth=1, style="filled,rounded", href="/reference/Interface.Plugin#options"];
    outputoptions [label="outputOptions", fillcolor="${#ffe8cc|#9d4f1a}", color="${#ff7e17|#cc5f1a}", penwidth=1, style="filled,rounded", href="/reference/Interface.Plugin#outputoptions"];
    buildstart [label="buildStart", fillcolor="${#ffcccc|#8a2a2a}", color="${#ff7e17|#cc5f1a}", penwidth=1, style="filled,rounded", href="/reference/Interface.Plugin#buildstart"];
    resolveid [label="resolveId", fillcolor="${#fff4cc|#9d7a1a}", color="${#ff7e17|#cc5f1a}", penwidth=1, style="filled,rounded", href="/reference/Interface.Plugin#resolveid"];
    load [label="load", fillcolor="${#fff4cc|#9d7a1a}", color="${#ff7e17|#cc5f1a}", penwidth=1, style="filled,rounded", href="/reference/Interface.Plugin#load"];
    transform [label="transform", fillcolor="${#ffe8cc|#9d4f1a}", color="${#ff7e17|#cc5f1a}", penwidth=1, style="filled,rounded", href="/reference/Interface.Plugin#transform"];
    moduleparsed [label="moduleParsed", fillcolor="${#ffcccc|#8a2a2a}", color="${#ff7e17|#cc5f1a}", penwidth=1, style="filled,rounded", href="/reference/Interface.Plugin#moduleparsed"];
    internaltransform [label="internalTransform", fillcolor="${#f0f0f0|#3a3a3a}", style="filled,rounded", color=transparent];
    resolvedynamicimport [label="resolveDynamicImport", fillcolor="${#fff4cc|#9d7a1a}", color="${#ff7e17|#cc5f1a}", penwidth=1, style="filled,rounded", href="/reference/Interface.Plugin#resolvedynamicimport"];
    buildend [label="buildEnd", fillcolor="${#ffcccc|#8a2a2a}", color="${#ff7e17|#cc5f1a}", penwidth=1, style="filled,rounded", href="/reference/Interface.Plugin#buildend"];

    // Main flow
    options -> outputoptions [penwidth=2];
    outputoptions -> buildstart [penwidth=2];
    buildstart -> resolveid [label="each entry", fontcolor="${#3c3c43|#dfdfd6}", penwidth=2];
    resolveid -> buildend [label="external", fontcolor="${#3c3c43|#dfdfd6}", style=dashed, penwidth=2];
    resolveid -> load [label="non-external", fontcolor="${#3c3c43|#dfdfd6}", penwidth=2];
    load -> transform [penwidth=2];
    transform -> internaltransform [penwidth=2];
    internaltransform -> moduleparsed [penwidth=2];
    moduleparsed -> buildend [label="no imports", fontcolor="${#3c3c43|#dfdfd6}", style=dashed, penwidth=2];
    moduleparsed -> resolvedynamicimport [label="each import()", fontcolor="${#3c3c43|#dfdfd6}", penwidth=2];
    resolvedynamicimport -> load [label="non-external", fontcolor="${#3c3c43|#dfdfd6}", penwidth=2];
    moduleparsed -> resolveid [label="each import", fontcolor="${#3c3c43|#dfdfd6}", penwidth=2];
    resolvedynamicimport -> buildend [label="external", fontcolor="${#3c3c43|#dfdfd6}", style=dashed, penwidth=2];
    resolvedynamicimport -> resolveid [label="unresolved", fontcolor="${#3c3c43|#dfdfd6}", penwidth=2];
    // Legend
    legend [shape=plaintext, style="", fillcolor=transparent, margin=0, fontsize=11, fontcolor="${#3c3c43|#dfdfd6}", label=<
        <table border="1" color="${#3c3c43|#dfdfd6}" style="rounded" cellborder="0" cellspacing="4" cellpadding="2">
            <tr><td colspan="2" align="right"><b>Legend</b></td></tr>
            <tr><td width="10" height="10" fixedsize="true" border="2" color="transparent" bgcolor="${#ffe8cc|#9d4f1a}"></td><td align="left">sequential</td></tr>
            <tr><td width="10" height="10" fixedsize="true" border="2" color="transparent" bgcolor="${#ffcccc|#8a2a2a}"></td><td align="left">parallel</td></tr>
            <tr><td width="10" height="10" fixedsize="true" border="2" color="transparent" bgcolor="${#fff4cc|#9d7a1a}"></td><td align="left">first</td></tr>
            <tr><td width="10" height="10" fixedsize="true" border="2" color="transparent" bgcolor="${#f0f0f0|#3a3a3a}"></td><td align="left">internal</td></tr>
            <tr><td width="10" height="10" fixedsize="true" border="2" color="${#3c3c43|#dfdfd6}" bgcolor="transparent"></td><td align="left">sync</td></tr>
            <tr><td width="10" height="10" fixedsize="true" border="2" color="${#ff7e17|#cc5f1a}" bgcolor="transparent"></td><td align="left">async</td></tr>
        </table>
    >];
    { rank=source; legend; }
}
```

Note that `internalTransform` in the graph above is not a plugin hook, it is the step where Rolldown transforms non-JS code to JS.

Additionally, in watch mode the [`watchChange`](/reference/Interface.Plugin#watchchange) hook can be triggered at any time to notify a new run will be triggered once the current run has generated its outputs. Also, when watcher closes, the [`closeWatcher`](/reference/Interface.Plugin#closewatcher) hook will be triggered.

> [!WARNING]
> **Unsupported Hooks**
>
> The following Build Hooks are supported by Rollup, but not by Rolldown:
>
> - `shouldTransformCachedModule` ([#4389](https://github.com/rolldown/rolldown/issues/4389))

### Output Generation Hooks

Output generation hooks can provide information about a generated bundle and modify a build once complete. Plugins that only use output generation hooks can also be passed in via the output options and therefore run only for certain outputs.

The first hook of the output generation phase is [`renderStart`](/reference/Interface.Plugin#renderstart), the last one is either [`generateBundle`](/reference/Interface.Plugin#generatebundle) if the output was successfully generated via [`bundle.generate(...)`](/reference/Interface.RolldownBuild#generate), [`writeBundle`](/reference/Interface.Plugin#writebundle) if the output was successfully generated via [`bundle.write(...)`](/reference/Interface.RolldownBuild#write), or [`renderError`](/reference/Interface.Plugin#rendererror) if an error occurred at any time during the output generation.

Additionally, [`closeBundle`](/reference/Interface.Plugin#closebundle) can be called as the very last hook, but it is the responsibility of the User to manually call [`bundle.close()`](/reference/Interface.RolldownBuild#close) to trigger this. The CLI will always make sure this is the case.

```dot
digraph {
    bgcolor="transparent";
    rankdir=TB;
    node [shape=box, style=filled, fontname="Arial", margin="0.2,0.1", color="${#3c3c43|#dfdfd6}", fontcolor="${#3c3c43|#dfdfd6}"];
    edge [fontname="Arial", color="${#3c3c43|#dfdfd6}"];

    // Node definitions with styling
    renderstart [label="renderStart", fillcolor="${#ffcccc|#8a2a2a}", color="${#3c3c43|#dfdfd6}", style="filled,rounded", href="/reference/Interface.Plugin#renderstart"];
    resolvefileurl [label="resolveFileUrl", fillcolor="${#fff4cc|#9d7a1a}", color="${#3c3c43|#dfdfd6}", style="filled,rounded", href="/reference/Interface.Plugin#resolvefileurl"];
    banner [label="banner", fillcolor="${#ffe8cc|#9d4f1a}", color="${#3c3c43|#dfdfd6}", style="filled,rounded", href="/reference/Interface.Plugin#banner"];
    footer [label="footer", fillcolor="${#ffe8cc|#9d4f1a}", color="${#3c3c43|#dfdfd6}", style="filled,rounded", href="/reference/Interface.Plugin#footer"];
    intro [label="intro", fillcolor="${#ffe8cc|#9d4f1a}", color="${#3c3c43|#dfdfd6}", style="filled,rounded", href="/reference/Interface.Plugin#intro"];
    outro [label="outro", fillcolor="${#ffe8cc|#9d4f1a}", color="${#3c3c43|#dfdfd6}", style="filled,rounded", href="/reference/Interface.Plugin#outro"];
    renderchunk [label="renderChunk", fillcolor="${#ffe8cc|#9d4f1a}", color="${#3c3c43|#dfdfd6}", style="filled,rounded", href="/reference/Interface.Plugin#renderchunk"];
    minify [label="minify", fillcolor="${#f0f0f0|#3a3a3a}", style="filled,rounded", color=transparent];
    postbanner [label="postBanner", fillcolor="transparent", color="${#3c3c43|#dfdfd6}", style="filled,rounded"];
    postfooter [label="postFooter", fillcolor="transparent", color="${#3c3c43|#dfdfd6}", style="filled,rounded"];
    augmentchunkhash [label="augmentChunkHash", fillcolor="${#ffe8cc|#9d4f1a}", color="${#ff7e17|#cc5f1a}", penwidth=1, style="filled,rounded", href="/reference/Interface.Plugin#augmentchunkhash"];
    generatebundle [label="generateBundle", fillcolor="${#ffe8cc|#9d4f1a}", color="${#3c3c43|#dfdfd6}", style="filled,rounded", href="/reference/Interface.Plugin#generatebundle"];
    writebundle [label="writeBundle", fillcolor="${#ffcccc|#8a2a2a}", color="${#3c3c43|#dfdfd6}", style="filled,rounded", href="/reference/Interface.Plugin#writebundle"];
    rendererror [label="renderError", fillcolor="${#ffcccc|#8a2a2a}", color="${#3c3c43|#dfdfd6}", style="filled,rounded", href="/reference/Interface.Plugin#rendererror"];
    closebundle [label="closeBundle", fillcolor="${#ffcccc|#8a2a2a}", color="${#3c3c43|#dfdfd6}", style="filled,rounded", href="/reference/Interface.Plugin#closebundle"];
    beforeimportmeta [label="", shape="circle", fixedsize="true", width=0.2, height=0.2, style="filled", fillcolor="${#3c3c43|#dfdfd6}", color=transparent];
    beforeaddons [label="", shape="circle", fixedsize="true", width=0.2, height=0.2, style="filled", fillcolor="${#3c3c43|#dfdfd6}", color=transparent];
    afteraddons [label="", shape="circle", fixedsize="true", width=0.2, height=0.2, style="filled", fillcolor="${#3c3c43|#dfdfd6}", color=transparent];

    // Main flow
    renderstart -> beforeimportmeta [label="each chunk", fontcolor="${#3c3c43|#dfdfd6}", penwidth=2];
    beforeimportmeta -> resolvefileurl [label="each import.meta.ROLLDOWN_FILE_URL_*", fontcolor="${#3c3c43|#dfdfd6}", penwidth=2];
    resolvefileurl -> beforeimportmeta [penwidth=2];
    beforeimportmeta -> beforeaddons [penwidth=2];
    augmentchunkhash -> generatebundle [penwidth=2];
    generatebundle -> writebundle [penwidth=2];
    writebundle -> closebundle [style=dashed, penwidth=2];
    afteraddons -> beforeimportmeta [label="next chunk", fontcolor="${#3c3c43|#dfdfd6}", style=dashed, constraint=false, penwidth=2];
    afteraddons -> renderchunk [label="each chunk", fontcolor="${#3c3c43|#dfdfd6}", penwidth=2];
    renderchunk -> minify [penwidth=2];
    minify -> postbanner [penwidth=2];
    minify -> postfooter [penwidth=2];
    postbanner -> augmentchunkhash [penwidth=2];
    postfooter -> augmentchunkhash [penwidth=2];
    augmentchunkhash -> renderchunk [label="next chunk", fontcolor="${#3c3c43|#dfdfd6}", style=dashed, constraint=false, penwidth=2];
    rendererror -> closebundle [style=dashed, penwidth=2];

    // Subgraphs
    subgraph cluster_generatechunks {
        style=invis;
        label="";

        beforeaddons -> banner [penwidth=2];
        beforeaddons -> footer [penwidth=2];
        beforeaddons -> intro [penwidth=2];
        beforeaddons -> outro [penwidth=2];
        banner -> afteraddons [penwidth=2];
        footer -> afteraddons [penwidth=2];
        intro -> afteraddons [penwidth=2];
        outro -> afteraddons [penwidth=2];
    }
    // Legend
    legend [shape=plaintext, style="", fillcolor=transparent, margin=0, fontsize=11, fontcolor="${#3c3c43|#dfdfd6}", label=<
        <table border="1" color="${#3c3c43|#dfdfd6}" style="rounded" cellborder="0" cellspacing="4" cellpadding="2">
            <tr><td colspan="2" align="right"><b>Legend</b></td></tr>
            <tr><td width="10" height="10" fixedsize="true" border="2" color="transparent" bgcolor="${#ffe8cc|#9d4f1a}"></td><td align="left">sequential</td></tr>
            <tr><td width="10" height="10" fixedsize="true" border="2" color="transparent" bgcolor="${#ffcccc|#8a2a2a}"></td><td align="left">parallel</td></tr>
            <tr><td width="10" height="10" fixedsize="true" border="2" color="transparent" bgcolor="${#fff4cc|#9d7a1a}"></td><td align="left">first</td></tr>
            <tr><td width="10" height="10" fixedsize="true" border="2" color="transparent" bgcolor="${#f0f0f0|#3a3a3a}"></td><td align="left">internal</td></tr>
            <tr><td width="10" height="10" fixedsize="true" border="2" color="${#3c3c43|#dfdfd6}" bgcolor="transparent"></td><td align="left">sync</td></tr>
            <tr><td width="10" height="10" fixedsize="true" border="2" color="${#ff7e17|#cc5f1a}" bgcolor="transparent"></td><td align="left">async</td></tr>
        </table>
    >];
    { rank=source; legend; }
}
```

Note that `minify` in the graph above is not a plugin hook and is the step where Rolldown runs the minifier. Also note that `postBanner` and `postFooter` are not plugin hooks, these are output options and do not have corresponding hooks, unlike `banner` and `footer`.

> [!WARNING]
> **Unsupported Hooks**
>
> The following Output Generation Hooks are supported by Rollup, but not by Rolldown:
>
> - `resolveImportMeta` ([#1010](https://github.com/rolldown/rolldown/issues/1010))
> - `renderDynamicImport` ([#4532](https://github.com/rolldown/rolldown/issues/4532))

## Plugin Context

A number of utility functions and informational bits can be accessed from within most hooks via `this`. See the [`PluginContext`](/reference/Interface.PluginContext) type for more information.

## Supporting TypeScript and JSX

To achieve optimal performance, Rolldown runs the internal transform which transforms TypeScript and JSX to JavaScript after the [`transform`](/reference/Interface.Plugin#transform) hooks are called. This means the plugins using `transform` hook need to support TypeScript and JSX. Basically, there are two ways to achieve this.

### Handling TypeScript and JSX Syntax

[`this.parse`](/reference/Interface.PluginContext#parse) supports parsing TypeScript and JSX by passing the `lang` option. This should allow the plugin to process TypeScript and JSX easily.

### Transforming TypeScript and JSX beforehand

If processing TypeScript and JSX AST is not an option, you can still transform them to JavaScript by using the `transform` function exposed from `rolldown/utils`. Note that this has an additional overhead.

## Notable Differences from Rollup

While Rolldown's plugin interface is largely compatible with Rollup's, there are some important behavioral differences to be aware of:

### Output Generation Handling

In Rollup, all outputs are generated together in a single process. However, Rolldown handles each output generation separately. This means that if you have multiple output configurations, Rolldown will process each output independently, which can affect how certain plugins behave, especially those that maintain state across the entire build process.

These are the concrete differences:

- [`outputOptions`](/reference/Interface.FunctionPluginHooks#outputoptions) hook is called **before** the build hooks in Rolldown, whereas Rollup calls them **after** the build hooks
- Build hooks are called for each output separately, whereas Rollup calls them once for all outputs
- [`closeBundle`](/reference/Interface.FunctionPluginHooks#closebundle) hook is called **only** when you called [`generate()`](/reference/Interface.RolldownBuild#generate) or [`write()`](/reference/Interface.RolldownBuild#write) at least once, whereas Rollup calls it regardless of whether you called `generate()` or `write()`

### Watch Mode Hook Behavior

In Rollup, the [`options`](/reference/Interface.Plugin#options) hook is called on every rebuild in watch mode. In Rolldown, the `options` hook is only called once when the watcher is created, and is not called again on subsequent rebuilds.

### Sequential Hook Execution

In Rollup, certain hooks like [`writeBundle`](/reference/Interface.FunctionPluginHooks#writebundle) are "parallel" by default, meaning they run concurrently across multiple plugins. This requires plugins to explicitly set `sequential: true` if they need their hooks to run one after another.

In Rolldown, the [`writeBundle`](/reference/Interface.FunctionPluginHooks#writebundle) hook is already sequential by default, so plugins do not need to specify `sequential: true` for this hook.

### Sourcemap Validation

Rollup does not check a plugin's sourcemap against its own `sources` and `names`. A mapping that points at a missing source is dropped. A mapping that points at a missing name is kept without the name. Rolldown checks every index while converting the map to the internal representation. So an invalid map that Rollup accepts can fail the build here. For example:

```
Failed to convert json sourcemap to struct
Reference to non-existing source at position 1
```
