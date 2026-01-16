# Plugin API

## Overview

Rolldown's plugin interface is almost fully compatible with Rollup's (detailed tracking [here](https://github.com/rolldown/rolldown/issues/819)), so if you have written a Rollup plugin before, you already know how to write a Rolldown plugin!

A Rolldown plugin is an object that satisfies the [plugin interface](#plugin-interface) described below.
A plugin should be distributed as a package which exports a function that can be called with plugin specific options and returns such an object.

Plugins allow you to customize Rolldown's behavior by, for example, transpiling code before bundling, or shimming a built-in module that is not available.

<!-- TODO: add a link to a guide on how to use plugins & how to find plugins -->

### Example

The following example shows a Rolldown plugin that intercepts import requests to `example-virtual-module` and returns a custom content for it.

::: code-group

```js [rolldown-plugin-example.js]
const id = 'example-virtual-module';
const resolvedId = '\0' + id;

export default function examplePlugin() {
  return {
    name: 'example-plugin', // this name will show up in logs and errors
    resolveId(source) {
      if (source === id) {
        // this signals to Rolldown that this import should resolve to a module named `\0example-virtual-module`
        return resolvedId;
      }
      return null; // other ids should be handled as usual
    },
    load(id) {
      if (id === resolvedId) {
        // the source code for `\0example-virtual-module`
        return `export default 'Hello from ${id}';`;
      }
      return null; // other ids should be handled as usual
    },
  };
}
```

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';
import examplePlugin from './rolldown-plugin-example.js';

export default defineConfig({
  plugins: [examplePlugin()],
});
```

:::

::: tip Virtual Modules {#virtual-modules}

This plugin implements a pattern which is commonly called "virtual modules".
A virtual module is a module that does not exist on the file system and is instead resolved and provided by a plugin.
In the example above, `example-virtual-module` is never read from disk because the plugin intercepts the import in `resolveId` and supplies the module’s source code in `load`.
This pattern is useful for injecting helper functions.

:::

::: warning Hook Filters

This example plugin does not use [Hook Filters](/apis/plugin-hook-filters) for simplicity.
To improve performance, it is recommended to use them when possible.

:::

## Conventions

- Plugins should have a clear name with `rolldown-plugin-` prefix.
- Include `rolldown-plugin` keyword in the package.json `keywords` field.
- Make sure your plugin outputs correct source mappings if appropriate.
- If your plugin uses ["virtual modules"](#virtual-modules), prefix the module ID with `\0`. This prevents other plugins from trying to process it.
- (recommended) Plugins should be tested.
- (recommended) Plugins should be documented in English.

<!-- TODO: add a guide how to test a plugin -->

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

```hooks-graph
# styles
sequential: fillcolor="#ffe8cc", dark$fillcolor="#9d4f1a"
parallel: fillcolor="#ffcccc", dark$fillcolor="#8a2a2a"
first: fillcolor="#fff4cc", dark$fillcolor="#9d7a1a"
internal: fillcolor="#f0f0f0", dark$fillcolor="#3a3a3a"
sync: color="#3c3c43", dark$color="#dfdfd6"
async: color="#ff7e17", dark$color="#cc5f1a", penwidth=1

# nodes
watchChange(/reference/Interface.Plugin#watchchange): parallel, async
closeWatcher(/reference/Interface.Plugin#closewatcher): parallel, async
options(/reference/Interface.Plugin#options): sequential, async
outputOptions(/reference/Interface.Plugin#outputoptions): sequential, async
buildStart(/reference/Interface.Plugin#buildstart): parallel, async
resolveId(/reference/Interface.Plugin#resolveid): first, async
load(/reference/Interface.Plugin#load): first, async
transform(/reference/Interface.Plugin#transform): sequential, async
moduleParsed(/reference/Interface.Plugin#moduleparsed): parallel, async
internalTransform: internal
resolveDynamicImport(/reference/Interface.Plugin#resolvedynamicimport): first, async
buildEnd(/reference/Interface.Plugin#buildend): parallel, async

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

Additionally, in watch mode the [`watchChange`](/reference/Interface.Plugin#watchchange) hook can be triggered at any time to notify a new run will be triggered once the current run has generated its outputs. Also, when watcher closes, the [`closeWatcher`](/reference/Interface.Plugin#closewatcher) hook will be triggered.

::: warning Unsupported Hooks

The following Build Hooks are supported by Rollup, but not by Rolldown:

- `shouldTransformCachedModule` ([#4389](https://github.com/rolldown/rolldown/issues/4389))

:::

### Output Generation Hooks

Output generation hooks can provide information about a generated bundle and modify a build once complete. Plugins that only use output generation hooks can also be passed in via the output options and therefore run only for certain outputs.

The first hook of the output generation phase is [`renderStart`](/reference/Interface.Plugin#renderstart), the last one is either [`generateBundle`](/reference/Interface.Plugin#generatebundle) if the output was successfully generated via [`bundle.generate(...)`](/reference/Interface.RolldownBuild#generate), [`writeBundle`](/reference/Interface.Plugin#writebundle) if the output was successfully generated via [`bundle.write(...)`](/reference/Interface.RolldownBuild#write), or [`renderError`](/reference/Interface.Plugin#rendererror) if an error occurred at any time during the output generation.

Additionally, [`closeBundle`](/reference/Interface.Plugin#closebundle) can be called as the very last hook, but it is the responsibility of the User to manually call [`bundle.close()`](/reference/Interface.RolldownBuild#close) to trigger this. The CLI will always make sure this is the case.

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
renderStart(/reference/Interface.Plugin#renderstart): parallel, sync
banner(/reference/Interface.Plugin#banner): sequential, sync
footer(/reference/Interface.Plugin#footer): sequential, sync
intro(/reference/Interface.Plugin#intro): sequential, sync
outro(/reference/Interface.Plugin#outro): sequential, sync
renderChunk(/reference/Interface.Plugin#renderchunk): sequential, sync
minify: internal
postBanner: option, sync
postFooter: option, sync
augmentChunkHash(/reference/Interface.Plugin#augmentchunkhash): sequential, async
generateBundle(/reference/Interface.Plugin#generatebundle): sequential, sync
writeBundle(/reference/Interface.Plugin#writebundle): parallel, sync
renderError(/reference/Interface.Plugin#rendererror): parallel, sync
closeBundle(/reference/Interface.Plugin#closebundle): parallel, sync
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

## Plugin Context

A number of utility functions and informational bits can be accessed from within most hooks via `this`. See the [`PluginContext`](/reference/Interface.PluginContext) type for more information.

## Notable Differences from Rollup

While Rolldown's plugin interface is largely compatible with Rollup's, there are some important behavioral differences to be aware of:

### Output Generation Handling

In Rollup, all outputs are generated together in a single process. However, Rolldown handles each output generation separately. This means that if you have multiple output configurations, Rolldown will process each output independently, which can affect how certain plugins behave, especially those that maintain state across the entire build process.

These are the concrete differences:

- [`outputOptions`](/reference/Interface.FunctionPluginHooks#outputoptions) hook is called **before** the build hooks in Rolldown, whereas Rollup calls them **after** the build hooks
- Build hooks are called for each output separately, whereas Rollup calls them once for all outputs
- [`closeBundle`](/reference/Interface.FunctionPluginHooks#closebundle) hook is called **only** when you called [`generate()`](/reference/Interface.RolldownBuild#generate) or [`write()`](/reference/Interface.RolldownBuild#write) at least once, whereas Rollup calls it regardless of whether you called `generate()` or `write()`

### Sequential Hook Execution

In Rollup, certain hooks like [`writeBundle`](/reference/Interface.FunctionPluginHooks#writebundle) are "parallel" by default, meaning they run concurrently across multiple plugins. This requires plugins to explicitly set `sequential: true` if they need their hooks to run one after another.

In Rolldown, the [`writeBundle`](/reference/Interface.FunctionPluginHooks#writebundle) hook is already sequential by default, so plugins do not need to specify `sequential: true` for this hook.

## File URLs

To reference a file URL reference from within JS code, use the `import.meta.ROLLUP_FILE_URL_referenceId` replacement. This will generate code that depends on the output format and generates a URL that points to the emitted file in the target environment. Note that the transformation assumes `URL` is available and `import.meta.url` is polyfilled except for CJS and ESM output formats.

The following example will detect imports of `.svg` files, emit the imported files as assets, and return their URLs to be used e.g. as the `src` attribute of an `img` tag:

::: code-group

```js [rolldown-plugin-svg-asset.js]
import path from 'node:path';
import fs from 'node:fs';

function svgResolverPlugin() {
  return {
    name: 'svg-resolver',
    resolveId: {
      filter: { id: /\.svg$/ },
      handler(source, importer) {
        return path.resolve(path.dirname(importer), source);
      },
    },
    load: {
      filter: { id: /\.svg$/ },
      handler(id) {
        const referenceId = this.emitFile({
          type: 'asset',
          name: path.basename(id),
          source: fs.readFileSync(id),
        });
        return `export default import.meta.ROLLUP_FILE_URL_${referenceId};`;
      },
    },
  };
}
```

```js [main.js (usage)]
import logo from '../images/logo.svg';
const image = document.createElement('img');
image.src = logo;
document.body.appendChild(image);
```

:::

Similar to assets, emitted chunks can be referenced from within JS code via `import.meta.ROLLUP_FILE_URL_referenceId` as well.

The following example will detect imports prefixed with `register-paint-worklet:` and generate the necessary code and separate chunk to generate a CSS paint worklet. Note that this will only work in modern browsers and will only work if the output format is set to `es`.

::: code-group

```js [rolldown-plugin-paint-worklet.js]
import { prefixRegex } from '@rolldown/pluginutils';
const REGISTER_WORKLET = 'register-paint-worklet:';

function registerPaintWorkletPlugin() {
  return {
    name: 'register-paint-worklet',
    load: {
      filter: { id: prefixRegex(REGISTER_WORKLET) },
      handler(id) {
        return `CSS.paintWorklet.addModule(
          import.meta.ROLLUP_FILE_URL_${this.emitFile({
            type: 'chunk',
            id: id.slice(REGISTER_WORKLET.length),
          })}
        );`;
      },
    },
    resolveId: {
      filter: { id: prefixRegex(REGISTER_WORKLET) },
      handler(source, importer) {
        // We remove the prefix, resolve everything to absolute ids and
        // add the prefix again. This makes sure that you can use
        // relative imports to define worklets
        return this.resolve(source.slice(REGISTER_WORKLET.length), importer).then(
          (resolvedId) => REGISTER_WORKLET + resolvedId.id,
        );
      },
    },
  };
}
```

```js [main.js (usage)]
import 'register-paint-worklet:./worklet.js';
import { color, size } from './config.js';
document.body.innerHTML += `<h1 style="background-image: paint(vertical-lines);">color: ${color}, size: ${size}</h1>`;
```

```js [worklet.js (usage)]
import { color, size } from './config.js';
registerPaint(
  'vertical-lines',
  class {
    paint(ctx, geom) {
      for (let x = 0; x < geom.width / size; x++) {
        ctx.beginPath();
        ctx.fillStyle = color;
        ctx.rect(x * size, 0, 2, geom.height);
        ctx.fill();
      }
    }
  },
);
```

```js [config.js (usage)]
export const color = 'greenyellow';
export const size = 6;
```

:::

If you build this code, both the main chunk and the worklet will share the code from `config.js` via a shared chunk. This enables us to make use of the browser cache to reduce transmitted data and speed up loading the worklet.

## Source Code Transformations

If a plugin transforms source code, it should generate a sourcemap automatically, unless there's a specific `sourceMap: false` option. Rolldown only cares about the `mappings` property (everything else is handled automatically). [magic-string](https://github.com/Rich-Harris/magic-string) provides a simple way to generate such a map for elementary transformations like adding or removing code snippets.

If it doesn't make sense to generate a sourcemap, return an empty sourcemap:

```js
return {
  code: transformedCode,
  map: { mappings: '' },
};
```

If the transformation does not move code, you can preserve existing sourcemaps by returning `null`:

```js
return {
  code: transformedCode,
  map: null,
};
```

## Inter-plugin communication

At some point when using many dedicated plugins, there may be the need for unrelated plugins to be able to exchange information during the build. There are several mechanisms through which Rolldown makes this possible.

### Custom resolver options

Assume you have a plugin that should resolve an import to different ids depending on how the import was generated by another plugin. One way to achieve this would be to rewrite the import to use special proxy ids, e.g. a transpiled import via `require("foo")` in a CommonJS file could become a regular import with a special id `import "foo?require=true"` so that a resolver plugin knows this.

The problem here, however, is that this proxy id may or may not cause unintended side effects when passed to other resolvers because it does not really correspond to a file. Moreover, if the id is created by plugin `A` and the resolution happens in plugin `B`, it creates a dependency between these plugins so that `A` is not usable without `B`.

Custom resolver option offer a solution here by allowing to pass additional options for plugins when manually resolving a module via [`this.resolve`](/reference/Interface.PluginContext#resolve). This happens without changing the id and thus without impairing the ability for other plugins to resolve the module correctly if the intended target plugin is not present.

```js
function requestingPlugin() {
  return {
    name: 'requesting',
    async buildStart() {
      const resolution = await this.resolve('foo', undefined, {
        custom: { resolving: { specialResolution: true } },
      });
      console.log(resolution.id); // "special"
    },
  };
}

function resolvingPlugin() {
  return {
    name: 'resolving',
    resolveId(id, importer, { custom }) {
      if (custom.resolving?.specialResolution) {
        return 'special';
      }
      return null;
    },
  };
}
```

Note the convention that custom options should be added using a property corresponding to the plugin name of the resolving plugin. It is responsibility of the resolving plugin to specify which options it respects.

### Custom module meta-data

Plugins can annotate modules with custom meta-data which can be set by themselves and other plugins via the [`resolveId`](/reference/Interface.Plugin#resolveid), [`load`](/reference/Interface.Plugin#load), and [`transform`](/reference/Interface.Plugin#transform) hooks and accessed via [`this.getModuleInfo`](/reference/Interface.PluginContext#getmoduleinfo), [`this.load`](/reference/Interface.PluginContext#load) and the [`moduleParsed`](/reference/Interface.Plugin#moduleparsed) hook. This meta-data should always be `JSON.stringify`-able and will be persisted in the cache e.g. in watch mode.

```js
function annotatingPlugin() {
  return {
    name: 'annotating',
    transform(code, id) {
      if (thisModuleIsSpecial(code, id)) {
        return { meta: { annotating: { special: true } } };
      }
    },
  };
}

function readingPlugin() {
  let parentApi;
  return {
    name: 'reading',
    buildEnd() {
      const specialModules = Array.from(this.getModuleIds()).filter(
        (id) => this.getModuleInfo(id).meta.annotating?.special,
      );
      // do something with this list
    },
  };
}
```

Note the convention that plugins that add or modify data should use a property corresponding to the plugin name, in this case `annotating`. On the other hand, any plugin can read all meta-data from other plugins via `this.getModuleInfo`.

If several plugins add meta-data or meta-data is added in different hooks, then these `meta` objects will be merged shallowly. That means if plugin `first` adds `{meta: {first: {resolved: "first"}}}` in the resolveId hook and `{meta: {first: {loaded: "first"}}}` in the load hook while plugin `second` adds `{meta: {second: {transformed: "second"}}}` in the `transform` hook, then the resulting `meta` object will be `{first: {loaded: "first"}, second: {transformed: "second"}}`. Here the result of the `resolveId` hook will be overwritten by the result of the `load` hook as the plugin was both storing them under its `first` top-level property. The `transform` data of the other plugin on the other hand will be placed next to it.

The `meta` object of a module is created as soon as Rolldown starts loading a module and is updated for each lifecycle hook of the module. If you store a reference to this object, you can also update it manually. To access the meta object of a module that has not been loaded yet, you can trigger its creation and loading the module via [`this.load`](/reference/Interface.PluginContext#load):

```js
function plugin() {
  return {
    name: 'test',
    buildStart() {
      // trigger loading a module. We could also pass an initial
      // "meta" object here, but it would be ignored if the module
      // was already loaded via other means
      this.load({ id: 'my-id' });
      // the module info is now available, we do not need to await
      // this.load
      const meta = this.getModuleInfo('my-id').meta;
      // we can also modify meta manually now
      meta.test = { some: 'data' };
    },
  };
}
```

### Direct plugin communication

For any other kind of inter-plugin communication, we recommend the pattern below. Note that `api` will never conflict with any upcoming plugin hooks.

```js
function parentPlugin() {
  return {
    name: 'parent',
    api: {
      //...methods and properties exposed for other plugins
      doSomething(...args) {
        // do something interesting
      },
    },
    // ...plugin hooks
  };
}

function dependentPlugin() {
  let parentApi;
  return {
    name: 'dependent',
    buildStart({ plugins }) {
      const parentName = 'parent';
      const parentPlugin = plugins.find((plugin) => plugin.name === parentName);
      if (!parentPlugin) {
        // or handle this silently if it is optional
        throw new Error(`This plugin depends on the "${parentName}" plugin.`);
      }
      // now you can access the API methods in subsequent hooks
      parentApi = parentPlugin.api;
    },
    transform(code, id) {
      if (thereIsAReasonToDoSomething(id)) {
        parentApi.doSomething(id);
      }
    },
  };
}
```
