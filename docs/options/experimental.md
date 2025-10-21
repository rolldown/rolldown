# Experimental Options

- **Type:** `object`
- **Default:** See individual options

Experimental features that may change in future releases and can introduce behavior change without a major version bump.

## strictExecutionOrder

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `experimental.strictExecutionOrder`

Lets modules be executed in the order they are declared. This is done by injecting runtime helpers to ensure that modules are executed in the order they are imported. External modules won't be affected.

### In-depth

:::warning
Enabling this option may negatively impact bundle size. It is recommended to use this option only when absolutely necessary.
:::

## disableLiveBindings

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `experimental.disableLiveBindings`

Disable live bindings for exported variables.

## viteMode

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `experimental.viteMode`

Enable Vite compatibility mode.

## resolveNewUrlToAsset

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `experimental.resolveNewUrlToAsset`

Resolve `new URL()` to asset references.

## hmr

- **Type:** `boolean | { host?: string; port?: number; implement?: string }`
- **Default:** `false`
- **Path:** `experimental.hmr`

Hot Module Replacement configuration.

### Examples

```js
export default {
  experimental: {
    hmr: {
      host: 'localhost',
      port: 3000,
    },
  },
};
```

## chunkModulesOrder

- **Type:** `'exec-order' | 'module-id'`
- **Default:** `'exec-order'`
- **Path:** `experimental.chunkModulesOrder`

Control which order to use when rendering modules in a chunk.

### In-depth

The available options:

- `'exec-order'`: Almost equivalent to the topological order of the module graph, but specially handling when module graph has cycle
- `'module-id'`: This is more friendly for gzip compression, especially for some javascript static asset lib (e.g. icon library)

:::info
Try to sort the modules by their module id if possible. Since Rolldown scope hoists all modules in the chunk, we only try to sort those modules by module id if we could ensure runtime behavior is correct after sorting.
:::

## attachDebugInfo

- **Type:** `'none' | 'simple' | 'full'`
- **Default:** `'simple'`
- **Path:** `experimental.attachDebugInfo`

Attach debug information to the output bundle.

### In-depth

The available modes:

- `'none'`: No debug information is attached
- `'simple'`: Attach comments indicating which files the bundled code comes from. These comments could be removed by the minifier
- `'full'`: Attach detailed debug information to the output bundle. These comments are using legal comment syntax, so they won't be removed by the minifier

:::warning
You shouldn't use `'full'` in the production build.
:::

## chunkImportMap

- **Type:** `boolean | { baseUrl?: string; fileName?: string }`
- **Default:** `false`
- **Path:** `experimental.chunkImportMap`

Enables automatic generation of a chunk import map asset during build.

### Examples

```js
export default {
  experimental: {
    chunkImportMap: {
      baseUrl: '/',
      fileName: 'importmap.json',
    },
  },
  plugins: [
    {
      name: 'inject-import-map',
      generateBundle(_, bundle) {
        const chunkImportMap = bundle['importmap.json'];
        if (chunkImportMap?.type === 'asset') {
          const htmlPath = path.resolve('index.html');
          let html = fs.readFileSync(htmlPath, 'utf-8');

          html = html.replace(
            /<script\s+type="importmap"[^>]*>[\s\S]*?<\/script>/i,
            `<script type="importmap">${chunkImportMap.source}</script>`,
          );

          fs.writeFileSync(htmlPath, html);
          delete bundle['importmap.json'];
        }
      },
    },
  ],
};
```

### In-depth

This map only includes chunks with hashed filenames, where keys are derived from the facade module name or primary chunk name. It produces stable and unique hash-based filenames, effectively preventing cascading cache invalidation caused by content hashes and maximizing browser cache reuse.

The output defaults to `importmap.json` unless overridden via `fileName`. A base URL prefix (default `"/"`) can be applied to all paths. The resulting JSON is a valid import map and can be directly injected into HTML via `<script type="importmap">`.

:::tip
If you want to learn more, you can check out the example here: [examples/chunk-import-map](https://github.com/rolldown/rolldown/tree/main/examples/chunk-import-map)
:::

## onDemandWrapping

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `experimental.onDemandWrapping`

Enable on-demand wrapping of modules.

## incrementalBuild

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `experimental.incrementalBuild`

Enable incremental build support. Required to be used with `watch` mode.

## transformHiresSourcemap

- **Type:** `boolean | 'boundary'`
- **Default:** `false`
- **Path:** `experimental.transformHiresSourcemap`

Enable high-resolution source maps for transform operations.

## nativeMagicString

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `experimental.nativeMagicString`

Use native Rust implementation of MagicString for source map generation.

### Examples

```js
export default {
  experimental: {
    nativeMagicString: true,
  },
  output: {
    sourcemap: true,
  },
};
```

### In-depth

[MagicString](https://github.com/rich-harris/magic-string) is a JavaScript library commonly used by bundlers for string manipulation and source map generation. When enabled, Rolldown will use a native Rust implementation of MagicString instead of the JavaScript version, providing significantly better performance during source map generation and code transformation.

**Benefits:**

- **Improved Performance**: The native Rust implementation is typically faster than the JavaScript version, especially for large codebases with extensive source maps
- **Background Processing**: Source map generation is performed asynchronously in a background thread, allowing the main bundling process to continue without blocking. This parallel processing can significantly reduce overall build times when working with JavaScript transform hooks
- **Better Integration**: Seamless integration with Rolldown's native Rust architecture

:::info
This is an experimental feature. While it aims to provide identical behavior to the JavaScript implementation, there may be edge cases. Please report any discrepancies you encounter.

For a complete working example, see [examples/native-magic-string](https://github.com/rolldown/rolldown/tree/main/examples/native-magic-string)
:::
