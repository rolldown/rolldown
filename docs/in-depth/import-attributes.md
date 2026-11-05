# Import Attributes

## Overview

Import attributes are the `with { ... }` clause of an import:

```js
import config from './config.json' with { type: 'json' };
const data = await import('./data.json', { with: { type: 'json' } });
```

Node.js and browsers require `with { type: 'json' }` to import JSON, and browsers require `with { type: 'css' }` for CSS module scripts. These imports are common in code written for these runtimes, including your dependencies.

With `experimental.importAttributes`, Rolldown treats the same file imported with different attributes as different modules. Plugins can read the attributes in `resolveId`, `load` and `transform`, and change them in `resolveId`. For the background of the design, see [RFC](https://github.com/rolldown/rolldown/discussions/11217).

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';

export default defineConfig({
  experimental: {
    importAttributes: true,
  },
});
```

::: warning Experimental
`experimental.importAttributes` is off by default. Turning it on changes the ids of modules imported with attributes. If your project uses custom plugins, read [Migrating plugins](#migrating-plugins) first. Also check [Limitations](#limitations).
:::

## Supported syntax

Rolldown reads attributes from static imports, re-exports and dynamic imports with a static `with` object:

| Syntax                                                  | `attributes`       |
| ------------------------------------------------------- | ------------------ |
| `import x from 'a' with { type: 'json' }`               | `{ type: 'json' }` |
| `export * from 'a' with { type: 'json' }`               | `{ type: 'json' }` |
| `import('a', { with: { type: 'json' } })`               | `{ type: 'json' }` |
| `import x from 'a' assert { type: 'json' }`             | `{ type: 'json' }` |
| `import('a', options)` where `options` is not a literal | `{}`               |
| `require()`, `new URL()`, CSS `@import`                 | `{}`               |

Rolldown treats the removed `assert` keyword as if it's `with` for compatibility, but this support will be removed in the future.

Rolldown reads attributes at build time, so the `import()` options must be written directly in the call. If they come from a variable, as in `import('./data.json', options)`, the attributes are not extracted.

## How it works

### Module ids

With the option on, import attributes become part of the module id. Plugins see three values for each module:

| Value        | Description                                                                                         |
| ------------ | --------------------------------------------------------------------------------------------------- |
| `rawId`      | The resolved id without attributes. This is the file to read and watch.                             |
| `attributes` | The import attributes as a `Record<string, string>`. `{}` means none.                               |
| `id`         | The module id, built from `rawId` and `attributes`. It equals `rawId` when there are no attributes. |

The id has the form `rawId?attributes=<encoded attributes>`. Rolldown sorts the attributes by key, serializes them with `JSON.stringify` and encodes the result with `encodeURIComponent`. If `rawId` already contains a `?`, the separator is `&`.

```js
import config from './config.json' with { type: 'json' };
import small from './logo.png' with { size: '100' };
import large from './logo.png' with { size: '400' };
import original from './logo.png';
```

| Import                               | `rawId`            | `attributes`       | `id`                                                        |
| ------------------------------------ | ------------------ | ------------------ | ----------------------------------------------------------- |
| `'./config.json'` `{ type: 'json' }` | `/src/config.json` | `{ type: 'json' }` | `/src/config.json?attributes=%7B%22type%22%3A%22json%22%7D` |
| `'./logo.png'` `{ size: '100' }`     | `/src/logo.png`    | `{ size: '100' }`  | `/src/logo.png?attributes=%7B%22size%22%3A%22100%22%7D`     |
| `'./logo.png'` `{ size: '400' }`     | `/src/logo.png`    | `{ size: '400' }`  | `/src/logo.png?attributes=%7B%22size%22%3A%22400%22%7D`     |
| `'./logo.png'` (no attributes)       | `/src/logo.png`    | `{}`               | `/src/logo.png` (unchanged)                                 |

::: warning Avoid `?attributes`
Avoid naming a query parameter `attributes` in your own ids. If its value happens to match this format, Rolldown and `parseModuleId` read it as import attributes.
:::

### External modules

In ESM output, imports of [external modules](/in-depth/external-modules) keep their attributes, including dynamic `import()`. This works with or without `experimental.importAttributes`. Without the option, an external imported with attributes still has them in its [module id](#module-ids), for example in `this.getModuleIds()`.

With the option on, a `resolveId` hook can change the attributes. If the [`external`](/reference/InputOptions.external) option marks an import as external before resolution, `resolveId` hooks do not run, so the import keeps the attributes written in the source.

For `cjs`, `iife` and `umd` output, see [Non ESM Output Formats](/in-depth/non-esm-output-formats#import-attributes).

### Built-in `type` attributes

With the option on, the `type` attribute selects one of Rolldown's [module types](/in-depth/module-types). Common cases work without a plugin:

| Attribute       | Module type | Default export |
| --------------- | ----------- | -------------- |
| `type: 'json'`  | `json`      | Parsed JSON    |
| `type: 'text'`  | `text`      | `string`       |
| `type: 'bytes'` | `binary`    | `Uint8Array`   |

```js
import config from './config.json' with { type: 'json' };
import shader from './shader.glsl' with { type: 'text' };
import wasm from './module.wasm' with { type: 'bytes' };
import data from './data.txt' with { type: 'json' };
```

Rolldown determines the module type by the following precedence, highest first:

1. The `moduleType` returned by a `load` hook.
2. The `type` attribute, when its value is `json`, `text` or `bytes`.
3. The file extension, including any mapping from the [`moduleTypes`](/reference/InputOptions.moduleTypes) option.

Other `type` values, such as `'css'`, have no built-in meaning and are passed to plugins unchanged. A plugin can handle them in its `load` hook.

## Plugin API

Without the option, the fields below still exist, but `attributes` is always `{}` and the `attributes` that `resolveId` returns are ignored.

### `resolveId`

```ts
resolveId(
  source: string,
  importer: string | undefined,
  extraOptions: {
    // ...
    // new: attributes of this import
    attributes: Record<string, string>;
    // new: `importer` without its attributes
    importerRawId: string | undefined;
    // new: attributes of the importer
    importerAttributes: Record<string, string>;
  },
): string | false | null | undefined | PartialResolvedId;

interface PartialResolvedId {
  // ...
  // new: replaces the attributes of the import
  attributes?: Record<string, string>;
}
```

Since `importer` contains the attributes of the importer, use `importerRawId` to get its file.

Return `attributes` to replace the attributes of the import, or `{}` to remove them. `load` and `transform` cannot change them.

For example, to add attributes to external JSON imports with the attribute Node.js requires:

```js
resolveId(source) {
  if (source.endsWith('.json')) {
    return { id: source, external: true, attributes: { type: 'json' } };
  }
},
// input:  import pkg from '../package.json'
// output: import pkg from '../package.json' with { type: "json" };
```

::: info Note
The `resolveDynamicImport` hook gets the same new options and can return `attributes` too.
:::

### `load`

```ts
load(
  id: string,
  // new
  options: {
    // `id` without attributes
    rawId: string;
    // attributes of the module
    attributes: Record<string, string>;
  },
): LoadResult;
```

### `transform`

```ts
transform(
  code: string,
  id: string,
  meta: {
    // ...
    // new: `id` without attributes
    rawId: string;
    // new: attributes of the module
    attributes: Record<string, string>;
  },
): TransformResult;
```

### `this.resolve`

```ts
this.resolve(
  source: string,
  // changed: also accepts `{ rawId, attributes }`
  importer?: string | { rawId: string; attributes?: Record<string, string> },
  options?: {
    // ...
    // new: attributes of the simulated import
    attributes?: Record<string, string>;
  },
): Promise<ResolvedId | null>;

interface ResolvedId {
  // ...
  // without attributes
  id: string;
  // new: final attributes
  attributes: Record<string, string>;
}
```

### `this.load`

```ts
this.load(options: {
  // ...
  // changed: also accepts `{ rawId, attributes }`
  id: string | { rawId: string; attributes?: Record<string, string> };
}): Promise<ModuleInfo>;
```

Attributes come only from `id`. To load a `this.resolve` result with its attributes, pass `{ rawId: resolved.id, attributes: resolved.attributes }` as `id`.

## Helper functions

Only `resolveId`, `load`, `transform`, `this.resolve` and `this.load` provide `rawId` and `attributes`. Everywhere else, plugins get the composite id:

| Where                                                                                             | Fields                                               |
| ------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| `this.getModuleInfo()`, `moduleParsed`                                                            | `id`, `importedIds`, `importers`, `dynamicImporters` |
| `this.getModuleIds()`                                                                             | Every returned id with import attributes             |
| Output chunks in `renderChunk`, `generateBundle`                                                  | `facadeModuleId`, `moduleIds`, the keys of `modules` |
| `onLog`, `onwarn`                                                                                 | `log.id`                                             |
| Option callbacks, such as the `test` of a [code splitting group](/in-depth/manual-code-splitting) | The module id                                        |

Use `parseModuleId` and `stringifyModuleId` from `rolldown/utils` to work with these ids:

```ts
function parseModuleId(id: string): { rawId: string; attributes: Record<string, string> };
function stringifyModuleId(rawId: string, attributes?: Record<string, string>): string;
```

```js
import { parseModuleId, stringifyModuleId } from 'rolldown/utils';

parseModuleId('/src/a.json?attributes=%7B%22type%22%3A%22json%22%7D');
// → { rawId: '/src/a.json', attributes: { type: 'json' } }

stringifyModuleId('/src/a.json', { type: 'json' });
// → '/src/a.json?attributes=%7B%22type%22%3A%22json%22%7D'

// warn about JSON files imported without `type: 'json'`
moduleParsed(info) {
  const { rawId, attributes } = parseModuleId(info.id);
  if (rawId.endsWith('.json') && attributes.type !== 'json') {
    this.warn(`${rawId} is imported without { type: 'json' }`);
  }
},
```

::: info Note
[`output.paths`](/reference/OutputOptions.paths), [`output.globals`](/reference/OutputOptions.globals) and [`treeshake.moduleSideEffects`](/reference/InputOptions.treeshake) receive `rawId`, not the composite id.
:::

## Migrating plugins

With the option on, plugins that treat a module id as a file path can break. The sections below cover the common cases and their fixes. In real projects, the most common trigger is JSON in Node-compatible code, such as `import pkg from './package.json' with { type: 'json' }`.

### Read files from `rawId`

`id` can end with an attributes suffix, so it is not always a file path. Read the file from `rawId`:

```js
// before: fails with ENOENT for a module with attributes
load(id) {
  return fs.readFile(id, 'utf8');
}

// after
load(id, { rawId }) {
  return fs.readFile(rawId, 'utf8');
}
```

The same applies to any code that uses the id as a path. `path.extname(id)` returns `.json?attributes=%7B...%7D`, and `this.addWatchFile(id)` watches a file that does not exist. `path.dirname(id)` still works, because `/` is encoded inside the attributes.

### Match the extension

A check on the end of `id` never matches a module with attributes. Check `rawId` instead:

```js
// before: never matches a module with attributes
transform(code, id) {
  if (!id.endsWith('.json')) return;
}

// after
transform(code, id, { rawId }) {
  if (!rawId.endsWith('.json')) return;
}
```

Hook filters match the composite id, so a filter that anchors the extension to the end of the id stops matching. Allow a query suffix:

```js
import { id, include, makeIdFiltersToMatchWithQuery } from '@rolldown/pluginutils';

// allow a query suffix by hand
filter: { id: /\.json($|\?)/ },

// `@rolldown/pluginutils` turns `/\.json$/` into `/\.json(?:\?.*)?$/`
filter: { id: makeIdFiltersToMatchWithQuery(/\.json$/) },

// composable filters: match the id with its query removed
filter: [include(id(/\.json$/, { cleanUrl: true }))],
```

For `json` and `text`, you can filter by `moduleType` instead. The module type is decided before `transform` runs, so an import with `type: 'json'` already has the module type `json`:

```js
transform: {
  filter: { moduleType: 'json' },
  handler(code, id, { rawId, attributes }) {
    // ...
  },
}
```

This does not work for `bytes`. Its module type is `binary`, and `transform` does not run for binary modules.

### Compare importers with `importerRawId`

If the importing module was itself imported with attributes, `importer` is its composite id. For example, a CSS file imported with `with { type: 'css' }` can contain `@import`. Use `importerRawId` to get the file:

```js
// before: `importer` may be `/src/style.css?attributes=...`
resolveId(source, importer) {
  if (!importer?.endsWith('.css')) return;
  // resolve `@import` and `url()` in CSS
}

// after
resolveId(source, importer, { importerRawId }) {
  if (!importerRawId?.endsWith('.css')) return;
  // ...
}
```

### Key state by the module id

A plugin often returns an id from `resolveId` and stores data under it for `load`. If the import has attributes, `load` gets that id with the attributes appended. Since a module with and without attributes are different modules, store the data under the full module id:

```js
import { stringifyModuleId } from 'rolldown/utils';

const sources = new Map();

resolveId(source, importer, { attributes }) {
  if (source.startsWith('virtual:')) {
    const id = '\0' + source;
    sources.set(stringifyModuleId(id, attributes), buildSource(source, attributes));
    return id;
  }
},
load(id) {
  return sources.get(id);
},
```

### Forward `this.resolve` and `this.load` calls

If your `resolveId` hook passes its arguments on to `this.resolve`, you do not need to change it. Rolldown reads the attributes from `importer` for you:

```js
resolveId(source, importer, options) {
  // importer: '/src/style.css?attributes=%7B%22type%22%3A%22css%22%7D'
  return this.resolve(source, importer, { ...options, skipSelf: true });
},
```

`this.resolve` returns the attributes in the `attributes` field. `this.load` only reads attributes from `id`. So to load the module with its attributes, pass `{ rawId, attributes }` as `id`:

```js
const resolved = await this.resolve('./data.js', importer, { attributes: { type: 'foo' } });
// resolved: { id: '/src/data.js', attributes: { type: 'foo' }, ... }
const info = await this.load({
  ...resolved,
  id: { rawId: resolved.id, attributes: resolved.attributes },
});
// info.id: '/src/data.js?attributes=%7B%22type%22%3A%22foo%22%7D'
```

Without the option, `resolved.attributes` is `{}` and `info.id` is `/src/data.js`. The same code works in both cases.

### Check magic queries

Magic queries, such as Vite's `?raw`, `?url` and `?inline`, share the query string with the attributes. An import that uses both keeps both in its id, so a check that anchors the query to the end of the id stops matching:

```js
// import svg from './icon.svg?raw' with { type: 'text' }
// id: '/src/icon.svg?raw&attributes=%7B%22type%22%3A%22text%22%7D'
id.endsWith('?raw'); // false
/[?&]raw\b/.test(id); // true
```

## Compatibility with Rollup 5

Rolldown follows the import attributes design of [Rollup 5](https://github.com/rollup/rollup/pull/6266). This section assumes `experimental.importAttributes` is on. Module ids have the same format, so a module gets the same id in both bundlers. The new hook fields, `this.resolve` and `this.load` behave the same, and `INVALID_IMPORT_ATTRIBUTE` is reported in the same cases.

Differences:

- Rolldown gives `type: 'json'`, `type: 'text'` and `type: 'bytes'` a built-in module type. Rollup 5 gives no attribute a built-in meaning.
- Rolldown always renders the attributes of external imports with `with`. Rollup 5 can drop them with `output.externalImportAttributes`, or use `assert` with the deprecated `output.importAttributesKey`.
- Rolldown sorts the keys of a rendered `with` clause. Rollup 5 keeps the order written in the source.
- For an `import()` of an external module, Rolldown keeps the options as written, unless a `resolveId` hook changed the attributes. Rollup 5 always rebuilds the options from the attributes, so it drops entries whose value is a variable.
- Rolldown warns about the legacy `assert` keyword. Rollup 5 accepts it without a warning.
- Rolldown does not support the deprecated `importerAttributes` option of `this.resolve()`. Pass `importer` as `{ rawId, attributes }` instead.

Not supported yet:

- `rawId` and `attributes` in `ModuleInfo` and `RenderedModule`
- `facadeModuleRawId` and `facadeModuleAttributes` in chunks
- The fourth argument of the `external` function, and `{ attributes }` for the `output.globals` and `output.paths` functions
- The `AMBIGUOUS_GLOBAL_NAME` warning, when the object form of `output.globals` gives several attribute variants of one external the same global name
- `this.getModuleInfo()` with a `{ rawId, attributes }` object, and `this.emitFile()` chunks with attributes
- `moduleRawId` and `moduleAttributes` in the `resolveFileUrl` hook

## Limitations

- Rolldown's dev mode is not supported yet.
- Hook filters cannot match attributes yet. Check `attributes` in the hook instead.
- Vite does not support this option yet. Its dev server and built-in plugins still need to be updated.
