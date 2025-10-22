# preserveEntrySignatures

- **Type:** `false | 'strict' | 'allow-extension' | 'exports-only'`
- **Default:** `'strict'`

Controls how entry chunk exports are preserved. This determines whether Rolldown needs to create facade chunks (additional wrapper chunks) to maintain the exact export signatures of entry modules, or whether it can combine entry modules with other chunks for optimization.

## Values

### `'exports-only'`

Follows `'strict'` behavior for entry modules that have exports, but allows `'allow-extension'` behavior for entry modules without exports. This provides a good balance between maintaining export signatures and optimization flexibility.

### `'strict'`

Entry chunks will exactly match the exports of their corresponding entry modules. If additional internal bindings need to be exposed (for example, when modules are shared between chunks), Rolldown will create facade chunks to maintain the exact export signature.

**Use case:** This is the recommended setting for **libraries** where you need guaranteed, stable export signatures.

### `'allow-extension'`

Entry chunks can expose all exports from the corresponding entry module, and may also include additional exports from other modules if they're bundled together. This allows more optimization opportunities but may expose internal implementation details.

### `false`

Provides maximum flexibility. Entry chunks can be merged freely with other chunks regardless of export signatures. This can lead to better optimization but may change the exposed exports significantly.

## Understanding Facade Chunks

A facade chunk is a small wrapper chunk that Rolldown creates to preserve the exact export signature of an entry module when the actual implementation has been bundled into another chunk.

**Example scenario:**

If you have two entry points that share code, and `preserveEntrySignatures` is set to `'strict'`, Rolldown might:

1. Bundle the shared code into a common chunk
2. Create facade chunks for each entry point that re-export from the common chunk
3. This ensures each entry point maintains its exact original export signature

## In-depth

### Override per Entry Point

The `preserveEntrySignatures` option is a global setting. The only way to override it for individual entry chunks is to use the plugin API and emit those chunks via `this.emitFile` instead of using the `input` option.

#### Practical Example: Mixed Library and Application Build

```js
// rolldown.config.js
export default {
  preserveEntrySignatures: 'exports-only', // Default for most entries
  plugins: [
    {
      name: 'custom-entries',
      buildStart() {
        // Library entry that needs strict signature preservation
        this.emitFile({
          type: 'chunk',
          id: 'src/library/index.js',
          fileName: 'library.js',
          preserveEntrySignature: 'strict',
        });

        // Application entry that can be optimized
        this.emitFile({
          type: 'chunk',
          id: 'src/app/main.js',
          fileName: 'app.js',
          preserveEntrySignature: false,
        });
      },
    },
  ],
};
```

When using `this.emitFile` with type `'chunk'`, you can specify:

- **`preserveEntrySignature`**: Override the global setting
  - `false`: Maximum optimization, merge chunks freely
  - `'strict'`: Exact export signature preservation
  - `'allow-extension'`: Allow additional exports from merged chunks
  - `'exports-only'`: Strict only for modules with exports

- **`fileName`**: Custom output filename for the entry chunk
- **`id`**: Module ID or path to use as the entry point

### When to Use Each Setting

- **`'strict'`**: Building libraries, need guaranteed export signatures
- **`'exports-only'`**: Most applications, balanced approach (default)
- **`'allow-extension'`**: Advanced optimizations, okay with exposing extra exports
- **`false`**: Maximum bundle size reduction, export signatures don't matter
