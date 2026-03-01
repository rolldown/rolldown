# Bundle Analyzer Plugin

The `bundleAnalyzerPlugin` is a built-in Rolldown plugin that analyzes bundle composition and emits a JSON report. The report contains chunk/module relationships, chunk imports, and per-entry reachability data.

:::tip NOTE
This plugin is currently exported from `rolldown/experimental` and may change in future releases.
:::

## Why Use This Plugin

While Rolldown's standard plugin API exposes low-level data — module importers, imported IDs, chunk composition, and chunk imports as filenames — building a complete bundle analysis from that data requires significant manual work: traversing the dependency graph, mapping filenames to chunk indices, and aggregating sizes.

Because the plugin is implemented as a Rust builtin, it can generate bundle metadata quickly even in large projects.

It emits a single, ready-to-consume JSON file with:

- **Module reachability from entry points** — computes which modules are reachable from each entry via DFS traversal, something the standard API does not provide.
- **Index-based relationships** — relationship fields (`moduleIndices`, `entryModule`, `targetChunkIndex`, `importers`) use compact numeric indices, making cross-references efficient for visualization tools.
- **Typed chunk import edges** — chunk-to-chunk import relationships are structured with an explicit `static`/`dynamic` type, rather than being split across separate arrays.
- **Chunk type classification** — each chunk is labeled as `"static-entry"`, `"dynamic-entry"`, or `"common"`, unifying the `isEntry`/`isDynamicEntry` booleans into a single field.
- **Pre-aggregated size data** — chunk and module sizes are included directly, no manual summation needed.

In short, the plugin provides a structured, visualization-ready bundle report that would otherwise require each tool author to reimplement the same graph traversal and data aggregation logic.

## Usage

Import and use the plugin from Rolldown's experimental exports:

```js
import { defineConfig } from 'rolldown';
import { bundleAnalyzerPlugin } from 'rolldown/experimental';

export default defineConfig({
  input: 'src/index.js',
  output: {
    dir: 'dist',
    format: 'esm',
  },
  plugins: [bundleAnalyzerPlugin()],
});
```

After building, the plugin emits `analyze-data.json` in the output directory.

## Options

### `fileName`

- **Type:** `string`
- **Default:** `"analyze-data.json"`

The output filename for the analysis data.

```js
bundleAnalyzerPlugin({
  fileName: 'bundle-analysis.json',
});
```

## Output Format

The generated JSON file has this shape:

```json
{
  "meta": {
    "bundler": "rolldown",
    "version": "1.0.0",
    "timestamp": 1700000000000
  },
  "chunks": [
    {
      "id": "chunk-main",
      "name": "main.js",
      "size": 1234,
      "type": "static-entry",
      "moduleIndices": [0, 1, 2],
      "entryModule": 0,
      "imports": [{ "targetChunkIndex": 1, "type": "dynamic" }],
      "reachableModuleIndices": [0, 1, 2, 3]
    }
  ],
  "modules": [
    {
      "id": "mod-0",
      "path": "src/index.js",
      "size": 256,
      "importers": [1]
    }
  ]
}
```

### `meta`

Metadata about the build:

| Field       | Type     | Description                         |
| ----------- | -------- | ----------------------------------- |
| `bundler`   | `string` | The bundler name (`"rolldown"`)     |
| `version`   | `string` | The bundler version                 |
| `timestamp` | `number` | Milliseconds since Unix epoch (UTC) |

### `chunks`

An array of chunk objects:

| Field                    | Type                    | Description                                                                                 |
| ------------------------ | ----------------------- | ------------------------------------------------------------------------------------------- |
| `id`                     | `string`                | Stable chunk ID in the format `chunk-{chunk.name}`                                          |
| `name`                   | `string`                | Output filename for the chunk                                                               |
| `size`                   | `number`                | Chunk code size in bytes                                                                    |
| `type`                   | `string`                | One of `"static-entry"`, `"dynamic-entry"`, or `"common"`                                   |
| `moduleIndices`          | `number[] \| undefined` | Indices into `modules` for modules present in this chunk                                    |
| `entryModule`            | `number \| undefined`   | Entry module index for entry chunks                                                         |
| `imports`                | `object[] \| undefined` | Import edges to other chunks (`{ targetChunkIndex, type }`, where `type` is static/dynamic) |
| `reachableModuleIndices` | `number[] \| undefined` | Modules reachable from `entryModule` (only for static/dynamic entry chunks)                 |

### `modules`

An array of module objects:

| Field       | Type                    | Description                                                          |
| ----------- | ----------------------- | -------------------------------------------------------------------- |
| `id`        | `string`                | Stable module ID in the format `mod-{index}`                         |
| `path`      | `string`                | Module path (absolute paths are converted to paths relative to cwd)  |
| `size`      | `number`                | Module source length in bytes                                        |
| `importers` | `number[] \| undefined` | Indices of modules that statically or dynamically import this module |

## Notes

- Absolute module IDs are normalized to relative paths from the current working directory for stable output.
- Virtual module IDs starting with `\0` are escaped as `\\0` in `modules[].path`.
