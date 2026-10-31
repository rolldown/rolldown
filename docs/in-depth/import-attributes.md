# Import Attributes

Import attributes are the `with { ... }` clause of an import:

```js
import config from './config.json' with { type: 'json' };
const data = await import('./data.json', { with: { type: 'json' } });
```

Node.js and browsers require `with { type: 'json' }` to import JSON, and browsers require `with { type: 'css' }` for CSS module scripts. Code written for these runtimes contains such attributes, so Rolldown has to understand them when it bundles that code.

With `experimental.importAttributes`, Rolldown makes import attributes part of module resolution and module identity, aligned with [Rollup 5](https://github.com/rollup/rollup/pull/6266). The same file imported with different attributes becomes different modules. Plugins see the attributes in `resolveId`, `load` and `transform`, and can change them.

This page explains the model and how to migrate a plugin to it. The design is specified in the [RFC](https://github.com/rolldown/rolldown/discussions/11217).

::: warning Experimental
`experimental.importAttributes` is off by default. It will be turned on by default in a later minor release, after plugins and Vite have had time to adapt. Module ids of modules imported with attributes change when it is on, so read the [migration section](#migrating-plugins) before enabling it in a project with custom plugins.
:::

## Enabling

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';

export default defineConfig({
  experimental: {
    importAttributes: true,
  },
});
```

With the option on:

- A module imported with attributes gets a [composite id](#module-ids) that contains the attributes.
- The `type` attribute selects a [built-in module type](#built-in-type-attributes) for `json`, `text` and `bytes`.
- Dynamic import options that Rolldown cannot analyze produce an [`INVALID_IMPORT_ATTRIBUTE`](#warnings) warning.

With the option off, the ids of internal modules do not change. The `resolveId` hook still receives the attributes of each import, so a plugin can be written against the new fields before the option is turned on. `load` and `transform` receive `{}` as attributes, because without composite ids one module can be imported with several different attribute sets.

[External modules](#external-modules) use their attributes in both modes, so their `with` clause is always kept.

## Module ids

Three terms matter, and they match Rollup 5:

- **`rawId`** is the resolved id without attributes. It is the file to read and watch.
- **`attributes`** are the import attributes as a `Record<string, string>`. `{}` means none.
- **`id`** is the module id. It is built from `rawId` and `attributes`, and equals `rawId` when there are no attributes.

The id is `rawId?attributes=<encoded attributes>`. The attributes are sorted by key, serialized with `JSON.stringify` and encoded with `encodeURIComponent`. If `rawId` already contains a `?`, `&` is used instead. The encoding is byte-identical to Rollup 5, so ids match across both bundlers.

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

Things to know about ids:

- A module without attributes keeps its id. Nothing changes for code that does not use import attributes.
- Key order does not matter. `{ type: 'image', size: '200' }` and `{ size: '200', type: 'image' }` are the same module.
- All attributes are kept, not only `type`. Attributes without a built-in meaning are passed to plugins as is.
- The same file imported with different attributes is bundled twice, runs its side effects twice and does not share state. This matches browsers, which also key modules by URL and type.
- Rolldown never splits an id by hand. `virtual?attributes=foo` is not valid encoded attributes, so it stays an opaque id. Plugins should not use `attributes` as a query key of their own.

Attributes are read from static imports, re-exports and dynamic imports with a static `with` object:

| Syntax                                                  | `attributes`                                   |
| ------------------------------------------------------- | ---------------------------------------------- |
| `import x from 'a' with { type: 'json' }`               | `{ type: 'json' }`                             |
| `export * from 'a' with { type: 'json' }`               | `{ type: 'json' }`                             |
| `import('a', { with: { type: 'json' } })`               | `{ type: 'json' }`                             |
| `import('a', options)` where `options` is not a literal | `{}`, with a warning                           |
| `import x from 'a' assert { type: 'json' }`             | `{ type: 'json' }`, with a warning (see below) |
| `require()`, `new URL()`, CSS `@import`                 | `{}`                                           |

The legacy `assert` keyword is deprecated. It was replaced by `with`, and Node.js 22 and Chrome 126 dropped it. Rolldown treats it as `with` and warns, so you should switch to `with`. The same goes for an `assert` key in the options of a dynamic import.

## Built-in `type` attributes

The `type` attribute selects one of Rolldown's [module types](/in-depth/module-types), so the most common cases work without a plugin:

| Attribute       | Module type | Default export | Status                                        |
| --------------- | ----------- | -------------- | --------------------------------------------- |
| `type: 'json'`  | `json`      | parsed JSON    | standard (ES2025 JSON modules)                |
| `type: 'text'`  | `text`      | `string`       | TC39 Stage 3, supported by esbuild, Deno, Bun |
| `type: 'bytes'` | `binary`    | `Uint8Array`   | TC39 Stage 2.7, supported by esbuild and Deno |

```js
import config from './config.json' with { type: 'json' };
import shader from './shader.glsl' with { type: 'text' }; // replaces Vite's `?raw`
import wasm from './module.wasm' with { type: 'bytes' };
import data from './data.txt' with { type: 'json' }; // overrides the extension
```

The module type is decided in this order:

1. The `moduleType` returned by a `load` hook. A plugin's choice always wins.
2. `attributes.type`, when it is `json`, `text` or `bytes`, even if the file extension says otherwise.
3. The file extension of `rawId`, including the [`moduleTypes`](/reference/InputOptions.moduleTypes) option. This is the behavior without import attributes.

Other values, such as `type: 'css'`, have no built-in meaning. They reach plugins unchanged, so a plugin can handle them in `load`.

## External modules

Attributes are rendered in ESM output for imports of [external modules](/in-depth/external-modules), including dynamic `import()`, with or without `experimental.importAttributes`:

- The import path is `rawId`. `?attributes=...` never appears in the output.
- The `with` clause uses the module's final attributes, after any change made by a `resolveId` hook.
- Imports of the same `rawId` with different attributes are different external modules, so each one gets its own import statement.

```js
// input
import a from 'ext' with { type: 'json' };
import b from 'ext';

// output
import a from 'ext' with { type: 'json' };
import b from 'ext';
```

Only ESM can express attributes. In the `cjs`, `iife` and `umd` formats, `require()` and globals carry no attributes, so they are dropped.

The [`external`](/reference/InputOptions.external) option is unchanged. When it marks an import as external before resolution, `resolveId` hooks do not run, and the import keeps the attributes written in the source.

## Migrating from earlier versions {#migrating-from-earlier-versions}

Rolldown 1.2 and earlier kept the `with` clause of an import of an external module as written. That was a partial implementation. Attributes were not part of module identity, so two imports of one external with different attributes were one module, and only one clause survived. For example, `import 'ext'; import 'ext' with { type: 'json' }` rendered only the second import.

Now an external uses its attributes as part of its identity, with or without `experimental.importAttributes`. Each set of attributes is its own external module and keeps its own clause. A build that imports each external with one set of attributes gets the same output as before. What changes is only visible to plugins: such an external module gets a [composite id](#module-ids) in `getModuleInfo()` and `moduleIds`, and `this.resolve()` returns its `attributes`. [`output.paths`](/reference/OutputOptions.paths), [`output.globals`](/reference/OutputOptions.globals) and `treeshake.moduleSideEffects` keep receiving the `rawId`.

Two things changed for every build, with or without the option:

- The legacy `assert` keyword is treated as `with` and the build warns. Use `with`.
- A rendered `with` clause is normalized: keys are sorted, duplicates are collapsed, and keys that are valid identifiers are not quoted.

## What changes in the plugin API

The new fields exist whether or not the option is on. Only their values depend on it, as described in [Enabling](#enabling).

### `resolveId`

```ts
resolveId(source: string, importer: string | undefined, options: {
  isEntry: boolean;
  kind: ImportKind;
  custom?: CustomPluginOptions;
  attributes: Record<string, string>;          // new: attributes of this import
  importerRawId: string | undefined;           // new: importer without attributes
  importerAttributes: Record<string, string>;  // new: the importer's own attributes
}): ResolveIdResult;

interface PartialResolvedId {
  id: string;                                  // the rawId
  external?: boolean | 'absolute' | 'relative';
  attributes?: Record<string, string>;         // new: replace the attributes, `{}` removes them
  // ...ModuleOptions
}
```

If the importing module was itself imported with attributes, `importer` is its composite id. Use `importerRawId` to get the file.

The built-in resolver ignores attributes, because they do not change which file a specifier resolves to.

### `load`

```ts
load(id: string, options: {
  rawId: string;                       // new
  attributes: Record<string, string>;  // new
}): LoadResult;
```

`id` is the composite id. Read the file from `options.rawId`. When no plugin returns code, Rolldown reads the file at `rawId` itself.

`load` cannot change attributes. They are fixed once the import is resolved. Use `resolveId` to change them.

### `transform`

```ts
transform(code: string, id: string, meta: {
  moduleType: ModuleType;
  magicString?: RolldownMagicString;
  ast?: Program;
  rawId: string;                       // new
  attributes: Record<string, string>;  // new
}): TransformResult;
```

### `this.resolve` and `this.load`

```ts
type UniqueModuleId = string | { rawId: string; attributes?: Record<string, string> };

this.resolve(source: string, importer?: UniqueModuleId, options?: {
  // ...existing options
  attributes?: Record<string, string>;  // new: attributes of the simulated import
}): Promise<ResolvedId | null>;

interface ResolvedId {
  id: string;                           // the rawId
  attributes: Record<string, string>;   // new: the final attributes
  // ...
}

this.load(options: {
  id: UniqueModuleId;                   // changed: also accepts { rawId, attributes }
  // ...existing options
}): Promise<ModuleInfo>;
```

A composite id string passed as `importer` or `id` is split into `rawId` and `attributes` automatically. As in Rollup 5, `this.load` takes the attributes only from `id`.

### Hook filters

[Hook filters](/apis/plugin-api/hook-filters) keep matching the composite `id`, as in Rollup 5. See [Match the extension](#match-the-extension) below for patterns that work with attributes.

### Helpers

```ts
import { parseModuleId, stringifyModuleId } from 'rolldown/utils';

function parseModuleId(id: string): { rawId: string; attributes: Record<string, string> };
function stringifyModuleId(rawId: string, attributes?: Record<string, string>): string;
```

`parseModuleId` only accepts a suffix that `stringifyModuleId` produces exactly, so an id that merely looks similar is returned as an opaque `rawId` with `{}` attributes.

## Migrating plugins

Composite ids break plugins that treat a module id as a file path. The breakages are predictable, and each has a small fix. The most common trigger in real projects is JSON in Node-compatible code, such as `import pkg from './package.json' with { type: 'json' }`.

### Read files from `rawId`

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

The same applies to anything that touches the file system or the path: `path.extname(id)` returns `.json?attributes=%7B...%7D`, and `this.addWatchFile(id)` watches a file that does not exist. `path.dirname(id)` still works, because `/` is encoded inside the attributes.

### Match the extension {#match-the-extension}

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

The `moduleType` filter is a good alternative for `json`, `text` and `bytes`, because the `type` attribute sets the module type before `transform` runs:

```js
transform: {
  filter: { moduleType: 'json' },
  handler(code, id, { rawId, attributes }) {
    // ...
  },
}
```

### Compare importers with `importerRawId`

```js
// before: `importer` may be `/src/a.json?attributes=...`
resolveId(source, importer) {
  if (importer?.endsWith('.json')) return;
}

// after
resolveId(source, importer, { importerRawId }) {
  if (importerRawId?.endsWith('.json')) return;
}
```

### Keep state by `rawId` or by the composite id, not both

A plugin that stores data under the id it returns from `resolveId` must look it up by `rawId` in `load` and `transform`, because `id` has the attributes appended there:

```js
const sources = new Map();

resolveId(source) {
  if (source.startsWith('virtual:')) {
    sources.set('\0' + source, buildSource(source));
    return '\0' + source;
  }
},
load(id, { rawId }) {
  return sources.get(rawId);
},
```

If the plugin needs one entry per attribute variant instead, key the map by the composite `id` in `load`.

### Use the helpers where only the composite id is available

`rawId` and `attributes` are exposed only in `resolveId`, `load`, `transform`, `this.resolve` and `this.load`. Everywhere else, plugins see the composite id:

- `this.getModuleInfo()` and `moduleParsed`: `id`, `importedIds`, `importers`, `dynamicImporters`
- `this.getModuleIds()`
- Output chunks in `renderChunk` and `generateBundle`: `facadeModuleId`, `moduleIds` and the keys of `modules`
- `onLog` and `onwarn`: `log.id`
- Option callbacks that match modules, such as the `test` of a [code splitting group](/in-depth/manual-code-splitting)

Do not parse these ids by hand. `id.split('?')[0]` or a `cleanUrl` helper also removes queries such as Vite's `?raw`, and skips the validity check. Use `parseModuleId` and `stringifyModuleId`:

```js
import { parseModuleId, stringifyModuleId } from 'rolldown/utils';

// list the real source files of a chunk
generateBundle(_, bundle) {
  for (const chunk of Object.values(bundle)) {
    if (chunk.type === 'chunk') {
      const files = new Set(chunk.moduleIds.map((id) => parseModuleId(id).rawId));
    }
  }
},

// look up a specific attribute variant
buildEnd() {
  const info = this.getModuleInfo(stringifyModuleId(rawId, { type: 'json' }));
},
```

Options that take a file path, such as [`output.paths`](/reference/OutputOptions.paths), [`output.globals`](/reference/OutputOptions.globals) and [`treeshake.moduleSideEffects`](/reference/InputOptions.treeshake), are called with `rawId`, as in Rollup.

### Forward `this.resolve` and `this.load` calls

The common forwarding pattern keeps working. `importer` already contains the attributes, and Rolldown splits it back into `rawId` and `attributes`:

```js
resolveId(source, importer, options) {
  // importer: '/src/a.json?attributes=%7B%22type%22%3A%22json%22%7D'
  return this.resolve(source, importer, { ...options, skipSelf: true });
},
```

To simulate an import with attributes, pass them in the options. The `id` of the result is the `rawId`, so passing the result to `this.load` as is loads the module without attributes. Put the attributes into `id` to load the module the import resolves to:

```js
const resolved = await this.resolve('./data.js', importer, { attributes: { type: 'foo' } });
// resolved: { id: '/src/data.js', attributes: { type: 'foo' }, ... }
const info = await this.load({
  ...resolved,
  id: { rawId: resolved.id, attributes: resolved.attributes },
});
// info.id: '/src/data.js?attributes=%7B%22type%22%3A%22foo%22%7D'
```

### Change attributes in `resolveId`

Return `attributes` to replace the attributes of an import, or `attributes: {}` to remove them. If you omit the field, the import keeps its own attributes.

A typical use is a library build for Node.js that keeps JSON imports external and adds the attribute Node.js requires:

```js
resolveId(source) {
  if (source.endsWith('.json')) {
    return { id: source, external: true, attributes: { type: 'json' } };
  }
},
// import pkg from '../package.json'
//   → import pkg from '../package.json' with { type: "json" };
```

### Watch out for magic queries

Plugins that use magic queries, such as Vite's `?raw`, `?url` and `?inline`, now share the query string with Rolldown. An import with both keeps both in its id, so checks that anchor the query to the end of the id stop matching:

```js
// import svg from './icon.svg?raw' with { type: 'text' }
// id: '/src/icon.svg?raw&attributes=%7B%22type%22%3A%22text%22%7D'
id.endsWith('?raw'); // false
/[?&]raw\b/.test(id); // true
```

### Checklist

- Read files and paths from `options.rawId` in `load`, and from `meta.rawId` in `transform`.
- Compare importers with `importerRawId`.
- Allow a query suffix in hook filters, or use `makeIdFiltersToMatchWithQuery` or `id(..., { cleanUrl: true })`.
- Use `parseModuleId` wherever only the composite id is available.
- Do not use `attributes` as a query key of your own.
- Test the plugin with `experimental.importAttributes: true` and an import that uses `with { type: 'json' }`.

## Warnings

Each warning has its own code and its own [`checks`](/reference/InputOptions.checks) option, so silencing one does not hide the others. Set the option to `false` to silence that warning.

| Code                          | Option                             | When                                                                                                                                                                                                                                                                                                                 |
| ----------------------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `DEPRECATED_IMPORT_ASSERTION` | `checks.deprecatedImportAssertion` | The legacy `assert` keyword is used, or, with the option on, an `assert` key in the options of a dynamic import. It is treated as `with`. Replace it with `with`.                                                                                                                                                    |
| `INVALID_IMPORT_ATTRIBUTE`    | `checks.invalidImportAttribute`    | With the option on, the options of a dynamic import cannot be analyzed. If they are not an object literal, or their `with` is not one, the import has no attributes. If an entry of the `with` object does not have a static string key and value, that entry is ignored. Rollup uses the same code for these cases. |

## Current limitations

- Vite's dev server and the dev engine do not use composite ids yet. A changed file maps to one module during HMR and incremental rebuilds. Vite will follow the same model, so plugins work the same in dev and build.
- Non-ESM output formats drop the attributes of external modules without a warning.
- Hook filters cannot match on attributes yet. Use the `moduleType` filter for the built-in types, or check `attributes` in the handler.
