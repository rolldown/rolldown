# output

Configuration options for the output bundle. These options control how Rolldown generates the final bundled files.

[[toc]]

## dir

- **Type:** `string`
- **Optional:** Yes ✅
- **Path:** `output.dir`

The directory where output files will be written. Required when using multiple entry points.

### Examples

```js
export default {
  input: {
    main: 'src/index.js',
    admin: 'src/admin.js',
  },
  output: {
    dir: 'dist',
  },
};
```

## file

- **Type:** `string`
- **Optional:** Yes ✅
- **Path:** `output.file`

The file path for the output bundle. Used for single entry builds.

### Examples

```js
export default {
  input: 'src/index.js',
  output: {
    file: 'dist/bundle.js',
  },
};
```

### In-depth

Use `file` for single-entry builds and `dir` for multi-entry builds.

## exports

- **Type:** `'auto' | 'named' | 'default' | 'none'`
- **Default:** `'auto'`
- **Path:** `output.exports`

Specifies what export mode to use for the bundle.

### Examples

```js
export default {
  output: {
    format: 'cjs',
    exports: 'named',
  },
};
```

### In-depth

The available modes:

- `'auto'`: Automatically determines the export mode based on the entry module's exports (recommended)
- `'named'`: Exports are exposed as named exports
- `'default'`: The entry module's default export is used as the main export
- `'none'`: No exports (useful for side-effect-only bundles)

## hashCharacters

- **Type:** `'base64' | 'base36' | 'hex'`
- **Default:** `'base64'`
- **Path:** `output.hashCharacters`

The character set to use for content hashes in file names.

### Examples

```js
export default {
  output: {
    hashCharacters: 'hex',
    entryFileNames: '[name]-[hash].js',
  },
};
```

### In-depth

The available character sets:

- `'base64'`: Uses base64 characters (shortest hashes)
- `'base36'`: Uses alphanumeric characters (0-9, a-z)
- `'hex'`: Uses hexadecimal characters (0-9, a-f)

## format

- **Type:** `'es' | 'cjs' | 'esm' | 'module' | 'commonjs' | 'iife' | 'umd'`
- **Default:** `'esm'`
- **Path:** `output.format`

Expected format of generated code.

### Examples

```js
export default {
  output: {
    format: 'esm',
  },
};
```

### In-depth

The available formats:

- `'es'`, `'esm'`, `'module'`: ES module format (uses `import` and `export`)
- `'cjs'`, `'commonjs'`: CommonJS format (uses `require()` and `module.exports`)
- `'iife'`: Immediately Invoked Function Expression (requires `name` option)
- `'umd'`: Universal Module Definition (requires `name` option)

## sourcemap

- **Type:** `boolean | 'inline' | 'hidden'`
- **Default:** `false`
- **Path:** `output.sourcemap`

Controls source map generation and configuration. See [output.sourcemap](./output-sourcemap.md) for detailed nested options.

### Examples

```js
export default {
  output: {
    sourcemap: true,
  },
};
```

## banner

- **Type:** `string | ((chunk: RenderedChunk) => string | Promise<string>)`
- **Optional:** Yes ✅
- **Path:** `output.banner`

Code to prepend to the beginning of each output chunk.

### Examples

```js
export default {
  output: {
    banner: '/* My Library v1.0.0 | MIT License */',
  },
};
```

## footer

- **Type:** `string | ((chunk: RenderedChunk) => string | Promise<string>)`
- **Optional:** Yes ✅
- **Path:** `output.footer`

Code to append to the end of each output chunk.

### Examples

```js
export default {
  output: {
    footer: '/* End of bundle */',
  },
};
```

## intro

- **Type:** `string | ((chunk: RenderedChunk) => string | Promise<string>)`
- **Optional:** Yes ✅
- **Path:** `output.intro`

Code to prepend inside the wrapper function (after banner, before actual code).

### Examples

```js
export default {
  output: {
    format: 'iife',
    intro: 'const ENVIRONMENT = "production";',
  },
};
```

## outro

- **Type:** `string | ((chunk: RenderedChunk) => string | Promise<string>)`
- **Optional:** Yes ✅
- **Path:** `output.outro`

Code to append inside the wrapper function (after actual code, before footer).

### Examples

```js
export default {
  output: {
    format: 'iife',
    outro: 'console.log("Bundle loaded");',
  },
};
```

## extend

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `output.extend`

When `true`, extends the global variable defined by the `name` option rather than overwriting it. Only applies to `iife` and `umd` formats.

### Examples

```js
export default {
  output: {
    format: 'umd',
    name: 'MyLib',
    extend: true,
  },
};
```

## esModule

- **Type:** `boolean | 'if-default-prop'`
- **Default:** `'if-default-prop'`
- **Path:** `output.esModule`

Controls whether to add a `__esModule: true` property when generating CJS output.

### Examples

```js
export default {
  output: {
    format: 'cjs',
    esModule: true,
  },
};
```

### In-depth

The available options:

- `false`: Never add `__esModule` property
- `true`: Always add `__esModule: true` property
- `'if-default-prop'`: Only add if there's a default export

## assetFileNames

- **Type:** `string | ((chunkInfo: PreRenderedAsset) => string)`
- **Default:** `'assets/[name]-[hash][extname]'`
- **Path:** `output.assetFileNames`

Pattern for naming asset files (non-JavaScript files).

Placeholders: `[name]`, `[hash]`, `[extname]`

### Examples

```js
export default {
  output: {
    assetFileNames: 'assets/[name].[hash][extname]',
  },
};
```

## entryFileNames

- **Type:** `string | ((chunkInfo: PreRenderedChunk) => string)`
- **Default:** `'[name].js'`
- **Path:** `output.entryFileNames`

Pattern for naming entry chunk files.

Placeholders: `[name]`, `[hash]`, `[ext]`

### Examples

```js
export default {
  output: {
    entryFileNames: '[name]-[hash].js',
  },
};
```

## chunkFileNames

- **Type:** `string | ((chunkInfo: PreRenderedChunk) => string)`
- **Default:** `'[name]-[hash].js'`
- **Path:** `output.chunkFileNames`

Pattern for naming non-entry chunk files (code-split chunks).

Placeholders: `[name]`, `[hash]`, `[ext]`

### Examples

```js
export default {
  output: {
    chunkFileNames: 'chunks/[name].[hash].js',
  },
};
```

## cssEntryFileNames

- **Type:** `string | ((chunkInfo: PreRenderedChunk) => string)`
- **Default:** `'[name].css'`
- **Path:** `output.cssEntryFileNames`

Pattern for naming CSS entry files.

Placeholders: `[name]`, `[hash]`

### Examples

```js
export default {
  output: {
    cssEntryFileNames: 'styles/[name]-[hash].css',
  },
};
```

## cssChunkFileNames

- **Type:** `string | ((chunkInfo: PreRenderedChunk) => string)`
- **Default:** `'[name]-[hash].css'`
- **Path:** `output.cssChunkFileNames`

Pattern for naming CSS chunk files (non-entry CSS files).

Placeholders: `[name]`, `[hash]`

### Examples

```js
export default {
  output: {
    cssChunkFileNames: 'styles/chunks/[name].[hash].css',
  },
};
```

## sanitizeFileName

- **Type:** `boolean | ((name: string) => string)`
- **Default:** `true`
- **Path:** `output.sanitizeFileName`

Controls sanitization of file names to ensure they're valid across different file systems.

### Examples

```js
export default {
  output: {
    sanitizeFileName: (name) => {
      return name.replace(/[^a-zA-Z0-9.-]/g, '_');
    },
  },
};
```

### In-depth

The available options:

- `false`: Disable sanitization
- `true`: Enable default sanitization
- Function: Custom sanitization logic

## minify

- **Type:** `boolean | 'dce-only' | MinifyOptions`
- **Default:** `false`
- **Path:** `output.minify`

Control code minification.

### Examples

```js
export default {
  output: {
    minify: true,
  },
};
```

### In-depth

The available options:

- `false`: Disable minification
- `true`: Enable full minification
- `'dce-only'`: Only dead code elimination
- Object: Fine-grained minification settings

## name

- **Type:** `string`
- **Optional:** Yes ✅
- **Path:** `output.name`

The global variable name for the bundle. Required when using `iife` or `umd` format.

### Examples

```js
export default {
  output: {
    format: 'iife',
    name: 'MyLibrary',
  },
};
```

## globals

- **Type:** `Record<string, string> | ((name: string) => string)`
- **Optional:** Yes ✅
- **Path:** `output.globals`

Maps external module IDs to global variable names for `iife` and `umd` formats.

### Examples

```js
export default {
  external: ['react', 'react-dom'],
  output: {
    format: 'iife',
    name: 'MyApp',
    globals: {
      'react': 'React',
      'react-dom': 'ReactDOM',
    },
  },
};
```

## paths

- **Type:** `Record<string, string> | ((id: string) => string)`
- **Optional:** Yes ✅
- **Path:** `output.paths`

Maps external module IDs to paths. Useful for loading dependencies from CDNs.

### Examples

```js
export default {
  external: ['d3', 'lodash'],
  output: {
    paths: {
      'd3': 'https://cdn.jsdelivr.net/npm/d3@7',
      'lodash': 'https://cdn.jsdelivr.net/npm/lodash@4',
    },
  },
};
```

## generatedCode

- **Type:** `Partial<GeneratedCodeOptions>`
- **Default:** `{}`
- **Path:** `output.generatedCode`

Configuration for the generated code output. See [output.generatedCode](./output-generated-code.md) for nested options.

## externalLiveBindings

- **Type:** `boolean`
- **Default:** `true`
- **Path:** `output.externalLiveBindings`

When `true`, generates code that supports live bindings for external imports.

### Examples

```js
export default {
  output: {
    externalLiveBindings: false,
  },
};
```

## inlineDynamicImports

- **Type:** `boolean`
- **Default:** `true` for 'iife' and 'umd' formats, `false` otherwise
- **Path:** `output.inlineDynamicImports`

When `true`, dynamic imports are inlined into the main bundle instead of being split into separate chunks.

### Examples

```js
export default {
  output: {
    inlineDynamicImports: true,
  },
};
```

## manualChunks

- **Type:** `(moduleId: string, meta: { getModuleInfo: (moduleId: string) => ModuleInfo | null }) => string | NullValue`
- **Optional:** Yes ✅
- **Path:** `output.manualChunks`

:::warning Deprecated
This option is deprecated. Please use [`output.advancedChunks`](./output-advanced-chunks.md) instead.
:::

Allows manual control over chunk creation.

## advancedChunks

- **Type:** `object`
- **Optional:** Yes ✅
- **Path:** `output.advancedChunks`

Advanced chunking configuration. See [output.advancedChunks](./output-advanced-chunks.md) for nested options.

## legalComments

- **Type:** `'none' | 'inline'`
- **Default:** `'inline'`
- **Path:** `output.legalComments`

Control comments in the output.

### Examples

```js
export default {
  output: {
    legalComments: 'inline',
  },
};
```

### In-depth

The available options:

- `'none'`: Remove all comments
- `'inline'`: Preserve comments containing `@license`, `@preserve`, or starting with `//!` or `/*!`

## polyfillRequire

- **Type:** `boolean`
- **Default:** `true`
- **Path:** `output.polyfillRequire`

When `true`, adds a polyfill for `require()` function in non-CommonJS formats.

### Examples

```js
export default {
  output: {
    format: 'esm',
    polyfillRequire: true,
  },
};
```

## hoistTransitiveImports

- **Type:** `boolean`
- **Default:** `true`
- **Path:** `output.hoistTransitiveImports`

When `false`, prevents hoisting transitive imports to entry chunks.

### Examples

```js
export default {
  output: {
    hoistTransitiveImports: false,
  },
};
```

## preserveModules

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `output.preserveModules`

When `true`, preserves the module structure instead of creating a single bundle.

### Examples

```js
export default {
  input: 'src/index.js',
  output: {
    dir: 'dist',
    preserveModules: true,
  },
};
```

## virtualDirname

- **Type:** `string`
- **Optional:** Yes ✅
- **Path:** `output.virtualDirname`

Virtual directory name for resolving `__dirname` and `__filename` in the output.

### Examples

```js
export default {
  output: {
    virtualDirname: '/virtual',
  },
};
```

## preserveModulesRoot

- **Type:** `string`
- **Optional:** Yes ✅
- **Path:** `output.preserveModulesRoot`

Base path to use when preserving modules. All preserved modules will be placed relative to this path.

### Examples

```js
export default {
  output: {
    dir: 'dist',
    preserveModules: true,
    preserveModulesRoot: 'src',
  },
};
```

## topLevelVar

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `output.topLevelVar`

When `true`, uses `var` declarations at the top level instead of function expressions.

### Examples

```js
export default {
  output: {
    topLevelVar: true,
  },
};
```

## cleanDir

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `output.cleanDir`

Clean the output directory ([`output.dir`](#dir)) before emitting output.

This feature cleans the output directory before `writeBundle` hooks are called. If you have advanced use cases like having multiple outputs with the same `output.dir`, we suggest you to run `rm` by yourself.

### Examples

```js
export default {
  output: {
    cleanDir: true,
  },
};
```

## minifyInternalExports

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `output.minifyInternalExports`

Whether to minify internal exports (exports not used by other chunks).

### Examples

```js
export default {
  output: {
    minifyInternalExports: true,
  },
};
```

## keepNames

- **Type:** `boolean`
- **Default:** `false`
- **Path:** `output.keepNames`

Preserve function and class names during bundling.

### Examples

```js
export default {
  output: {
    keepNames: true,
  },
};
```

### In-depth

When enabled, the bundler will preserve the original names of functions and classes in the output, which is useful for:

- Debugging and error stack traces
- Code that relies on `Function.prototype.name`
- Serialization that depends on constructor names
