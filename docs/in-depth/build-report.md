# Build Report

With `devtools: { mode: 'report' }`, Rolldown writes a build report: a folder of small Markdown and JSON files about each build. The report shows where the build time goes, what the output contains, and what changed since the previous build. You can read it yourself, use its numbers in scripts, or give the folder to a coding agent.

:::tip EXPERIMENTAL
The `devtools` option is experimental, and the report format may change in future releases. The report needs Rolldown 1.2.16 or later.
:::

## Usage

::: code-group

```js [rolldown.config.js]
import { defineConfig } from 'rolldown';

export default defineConfig({
  input: 'src/main.js',
  devtools: { mode: 'report' },
});
```

```js [vite.config.js]
import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    rolldownOptions: {
      devtools: { mode: 'report' },
    },
  },
});
```

```sh [CLI]
$ rolldown src/main.js -d dist --devtools.mode report
```

:::

Vite 8 builds with Rolldown. Run `npm ls rolldown` to see which Rolldown version your project uses.

## Where the report goes

Rolldown writes the report to `node_modules/.rolldown/report/<output dir>/`, relative to [`cwd`](/reference/InputOptions.cwd). `<output dir>` is the output folder (`output.dir`, the folder of `output.file`, or `dist`) relative to `cwd`. Each `..` in it becomes `__`:

| Output folder | Report folder                           |
| ------------- | --------------------------------------- |
| `dist`        | `node_modules/.rolldown/report/dist/`   |
| `../out`      | `node_modules/.rolldown/report/__/out/` |

Each output folder keeps its own report, so a build compares against the previous build of the same output. One bundle writes one report: when a bundle writes several outputs, the first output picks the folder, and the report describes the last output.

Rolldown writes the report when the bundle closes. `build()`, the CLI and `vite build` close it for you. With the [`rolldown()`](/reference/Function.rolldown) API, call `bundle.close()`:

```js
import { rolldown } from 'rolldown';

const bundle = await rolldown({
  input: 'src/main.js',
  devtools: { mode: 'report' },
});
await bundle.write({ dir: 'dist' });
await bundle.close(); // Rolldown writes the report here
```

## Files

Start with `entry.md`. It summarizes the build and tells you which file to open next.

| File                | Contents                                                                                                                                                                         |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `entry.md`          | Build time, output size, module, chunk and package counts, and the change since the previous build.                                                                              |
| `timing.md`         | Scan and generate times, plugin timings for `resolveId`, `load`, `transform` and `renderChunk`, the hooks that did no work, and the slowest modules in plugin `transform` hooks. |
| `chunks.md`         | The largest chunks, the reason each chunk exists, and modules duplicated across chunks.                                                                                          |
| `modules.md`        | Import kinds, the most-imported modules, and modules shared by several entries.                                                                                                  |
| `packages.md`       | The largest packages, direct and transitive dependencies, and packages with more than one version.                                                                               |
| `graph.md`          | Entry points, the chunk import graph, the initial load of each entry, and retained sizes.                                                                                        |
| `delta.md`          | The change since the previous build and since the pinned baseline. Written from the second build on.                                                                             |
| `metrics.json`      | The numbers of the build, with stable metric ids such as `output.total_bytes` and `build.total_ms`. Use it in scripts.                                                           |
| `module-graph.json` | The full module graph: every module with its size, its imports and its retained size.                                                                                            |
| `chunk-graph.json`  | Every output chunk with its file name, size, entry flag and imports. Rolldown 1.2.16 does not write it.                                                                          |
| `history.jsonl`     | One summary line per build.                                                                                                                                                      |
| `AGENTS.md`         | Instructions for a coding agent that reads the folder.                                                                                                                           |
| `.state.json`       | A snapshot of this build. The next build compares against it.                                                                                                                    |

- Chunk and asset sizes are measured before `generateBundle` hooks, before compression, and exclude sourcemap files. Files added or changed by `generateBundle` or `writeBundle` hooks are not reflected in these sizes.
- Module and package sizes count rendered module code after tree shaking, before `renderChunk` hooks and chunk minification. A module duplicated across chunks is counted once per copy.
- The initial load of an entry is its chunk plus every chunk that it imports statically, directly or through other chunks.
- The retained size of a module estimates the rendered module bytes that could leave the static initial load if all eager import paths to it were deferred: the module itself plus the modules that only it brings in. It is calculated across all user-defined entries, and is not a prediction of compressed download savings or load time.
- Direct packages are those imported by project modules under `cwd` and outside `node_modules`; other packages are classified as transitive. This classification uses the import graph, not `package.json` dependency fields.
- Most lists keep the top 20 items. `module-graph.json` lists every module, and `chunk-graph.json` every chunk.
- Plugin timings cover `resolveId`, `load`, `transform` and `renderChunk`. They add up calls that run at the same time, so they can be larger than the build time. Core TypeScript and JSX transformation is not attributed per module in the transform hotspots.
- A hook call did no work when `resolveId`, `load` or `renderChunk` returned nothing, or `transform` returned nothing or the code unchanged. For a JavaScript plugin, each such call still crosses from Rust to JavaScript. A [hook filter](/apis/plugin-api/hook-filters) skips the call, and a skipped call does not count. Builtin plugins (`builtin:*`) run in Rust, so their no-op calls cost little. Rolldown 1.2.16 shows only the No-op column of the hook table, and it also counts the calls that the filter skips.

## Compare builds

Each build compares against the previous build of the same output folder. To compare against a fixed build, pin it as the baseline:

```sh
$ cd node_modules/.rolldown/report/dist
$ cp .state.json baseline.json
```

Every later build then also compares against the baseline, in `delta.md` and in `metrics.json` (`baselineDelta`). Delete `baseline.json` to unpin it.

## Limits

- Only one-off builds write a report. [`watch()`](/reference/Function.watch), `rolldown --watch` and the dev engine (`experimental.devMode`) write none.
- A build writes the report or the full devtools log (`mode: 'full'`, the default), not both.
- The report is inside `node_modules`, so a clean install deletes it, together with the pinned baseline.

## Related tools

- The default `'full'` mode writes the complete devtools event log, which `@rolldown/debug` and Vite DevTools read. On a large app, the log can be several gigabytes. The report is a small summary of the same events.
- [`bundleAnalyzerPlugin`](/builtin-plugins/bundle-analyzer) writes one chunk and module analysis file into the output folder. The report adds build and plugin timings, package data and build-over-build changes, and it stays out of the output folder.
- [`@rolldown/perf-kit`](/in-depth/perf-kit) measures page load, server startup and Cloudflare Worker startup for a coding agent that makes your app start faster. It reads `module-graph.json` to find code that can load later, and `chunk-graph.json` to find chunks that start late.
