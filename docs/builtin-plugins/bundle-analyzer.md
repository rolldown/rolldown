# Bundle Analyzer Plugin

The `bundleAnalyzerPlugin` is a built-in Rolldown plugin that emits a detailed report describing your bundle's chunks, modules, dependencies, and reachability information. The report can be consumed by visualization tools, custom scripts, or LLM-based coding agents.

:::tip EXPERIMENTAL
This plugin is currently experimental and is exported from `rolldown/experimental`. Its API may change in future releases.
:::

## Usage

Import and use the plugin from Rolldown's experimental exports:

```js
import { defineConfig } from 'rolldown';
import { bundleAnalyzerPlugin } from 'rolldown/experimental';

export default defineConfig({
  input: 'src/main.js',
  output: {
    dir: 'dist',
    format: 'esm',
  },
  plugins: [bundleAnalyzerPlugin()],
});
```

After running the build, the plugin emits an analysis file alongside your bundled output (by default `dist/analyze-data.json`).

## Options

### `fileName`

- **Type:** `string`
- **Default:** `'analyze-data.json'` when `format` is `'json'`, `'analyze-data.md'` when `format` is `'md'`

The filename used for the emitted analysis asset. The file is emitted into the same output directory as the rest of the bundle.

```js
bundleAnalyzerPlugin({
  fileName: 'bundle-analysis.json',
});
```

### `format`

- **Type:** `'json' | 'md'`
- **Default:** `'json'`

Selects the output format.

- `'json'` produces a structured data file suitable for programmatic analysis or third-party visualizers.
- `'md'` produces a markdown report tailored for LLM consumption (see [Markdown Format](#markdown-format) below).

```js
bundleAnalyzerPlugin({
  format: 'md',
});
```

## JSON Format

When `format` is `'json'` (the default), the emitted file contains a structured object with the shape below. The `timestamp` field is milliseconds since the Unix epoch.

```jsonc
{
  "meta": {
    "bundler": "rolldown",
    "version": "1.0.0",
    "timestamp": 1705314645123,
  },
  "chunks": [
    {
      "id": "chunk-main",
      "name": "main-abc123.js",
      "size": 45230,
      "type": "static-entry", // or "dynamic-entry" or "common"
      "moduleIndices": [0, 1, 2],
      "entryModule": 0,
      "imports": [
        {
          "targetChunkIndex": 1,
          "type": "static", // or "dynamic"
        },
      ],
      "reachableModuleIndices": [0, 1, 2, 3, 4],
    },
  ],
  "modules": [
    {
      "id": "mod-0",
      "path": "src/main.js",
      "size": 3450,
      "importers": [1, 2],
    },
  ],
}
```

The JSON output can be uploaded to community visualizers such as [chunk-visualize](https://iwanabethatguy.github.io/chunk-visualize/), or processed by custom scripts to track bundle metrics over time.

## Markdown Format

When `format: 'md'` is set, the plugin emits a structured markdown report instead of JSON. The report is designed to be consumed by LLM-based coding agents, so you can pipe it directly into a prompt for review and refactoring suggestions.

The report is organized into the following sections:

| Section                                    | Description                                                                                                           |
| ------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| **Quick Summary**                          | Total output size, input module count, entry points, and number of code-split (common) chunks.                        |
| **Largest Modules by Output Contribution** | All modules sorted by size, with each module's percentage share of the total output.                                  |
| **Entry Point Analysis**                   | For each entry: its output filename, bundle size, the chunks it loads, and the modules it bundles.                    |
| **Dependency Chains**                      | Modules imported by multiple files, useful for understanding why a module ends up in the bundle.                      |
| **Optimization Suggestions**               | Actionable suggestions with severity levels (see below).                                                              |
| **Full Module Graph**                      | Complete per-module dependency information (imports, imported-by, size).                                              |
| **Raw Data for Searching**                 | Grep-friendly lines using `[MODULE:]`, `[OUTPUT_BYTES:]`, `[IMPORT:]`, `[IMPORTED_BY:]`, `[ENTRY:]`, `[CHUNK:]` tags. |

### Optimization Suggestions

The suggestions section identifies modules that live in **shared common chunks** but are only reachable from a **single static entry**. Such modules are unnecessarily shared and could be moved closer to their entry point by enabling [`entriesAware: true`](../reference/TypeAlias.CodeSplittingGroup.md#entriesaware) on your [`output.codeSplitting`](../reference/OutputOptions.codeSplitting.md) groups, which is the same fix the report's own optimization tip recommends.

Each suggestion is tagged with a severity level based on the proportion of single-entry-reachable module size within the common chunk:

- `[HIGH]`: greater than 50%
- `[MEDIUM]`: between 30% and 50%
- `[LOW]`: less than 30%

### Piping the Report into an LLM

Because the report is plain markdown, you can feed it directly to an AI assistant for review:

```bash
# After running your build
cat dist/analyze-data.md | your-cli-coding-agent "review this bundle and suggest improvements"
```

## Example

A runnable example is available in the [`examples/bundle-analyzer-demo`](https://github.com/rolldown/rolldown/tree/main/examples/bundle-analyzer-demo) directory of the Rolldown repository. It demonstrates a multi-entry project that produces interesting optimization suggestions when analyzed with `format: 'md'`.
