# Build Report

With `devtools: { mode: 'report' }`, Rolldown writes a build report: a folder of small Markdown and JSON files about each build. The report shows how the build uses its time, what is in the output, and the changes since the previous build. You can give the folder to a coding agent, read it yourself, or use its numbers in scripts.

::: warning Experimental
The `devtools` option is experimental. The report format can be different in later releases. The report needs Rolldown 1.2.16 or later.
:::

## Why a report

The default `'full'` mode writes every build event to a log, with the source text of each module, for visual tools such as Vite DevTools. A coding agent does not need a visual view. With only this log, the agent must write scripts to extract numbers from a file that can reach several gigabytes on a large app. This work uses tokens and space in the context window.

We designed the report for coding agents. Rolldown calculates the numbers, such as plugin timings, build changes and retained sizes. The agent reads the result. An agent starts with `entry.md` (1.6 KB in the build below) and opens other files only when it needs them. People and scripts can use the report too.

This table shows one build of [`apps/10000`](https://github.com/rolldown/benchmarks/tree/main/apps/10000) from the benchmark repository:

| `devtools`       | Data written | Build time |
| ---------------- | ------------ | ---------- |
| Off              | None         | 1.18 s     |
| `mode: 'report'` | 4.5 MB       | 1.59 s     |
| `mode: 'full'`   | 185 MB       | 1.59 s     |

Build time is the median of 5 builds on an Apple M3 Max. It includes Node.js startup. Both modes collect the same events, so both make the build slower. Turn on the report only when you investigate a build.

Most of the 4.5 MB is `module-graph.json`. Agents and scripts query this file and do not read all of it. All the Markdown files together are 14 KB, because most lists keep only the top 20 items.

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

Vite 8 uses Rolldown. To see which Rolldown version your project uses, run `npm ls rolldown`.

## Where the report goes

Rolldown writes the report to `node_modules/.rolldown/report/<output dir>/`, relative to [`cwd`](/reference/InputOptions.cwd). `<output dir>` is the output folder (`output.dir`, the folder of `output.file`, or `dist`) relative to `cwd`. Each `..` in it becomes `__`:

| Output folder | Report folder                           |
| ------------- | --------------------------------------- |
| `dist`        | `node_modules/.rolldown/report/dist/`   |
| `../out`      | `node_modules/.rolldown/report/__/out/` |

Each output folder keeps its own report.

Rolldown writes the report when the bundle closes. `build()`, the CLI and `vite build` close the bundle. With the [`rolldown()`](/reference/Function.rolldown) API, use `bundle.close()`:

```js
import { rolldown } from 'rolldown';

const bundle = await rolldown({
  input: 'src/main.js',
  devtools: { mode: 'report' },
});
await bundle.write({ dir: 'dist' });
await bundle.close(); // Rolldown writes the report here
```

## Read a report

This example adds one plugin to `apps/10000`. The plugin handles only `.css` files, but it has no [hook filter](/apis/plugin-api/hook-filters), so Rolldown runs it for every module. The report text below comes from the code for Rolldown 1.3.0. Rolldown 1.2.16 writes less.

```js [rolldown.config.js]
export default defineConfig({
  // ... the config of the app
  devtools: { mode: 'report' },
  plugins: [
    {
      name: 'my-css-plugin',
      transform(code, id) {
        if (!id.endsWith('.css')) return;
        // ... transform CSS
      },
    },
  ],
});
```

### Start with `entry.md`

The summary shows the main numbers and the file to open next:

```text [entry.md]
Build 843.6 ms · output 1.8 MB · 20014 modules (10002 external) / 1 chunk / 1 asset / 4 packages · browser / es · build #1

9 plugin hooks did no work in any call — a hook filter would skip those calls (`timing.md`, "Hooks that did no work")
```

### Find the cause in `timing.md`

These are the first rows of the hook table:

| Plugin                | Hook      | Calls | No-op        | Time     |
| --------------------- | --------- | ----- | ------------ | -------- |
| `my-css-plugin`       | transform | 10012 | 10012 (100%) | 307.99 s |
| `builtin:data-url`    | resolveId | 50013 | 50013 (100%) | 812.4 ms |
| `builtin:oxc-runtime` | resolveId | 50013 | 50013 (100%) | 763.7 ms |

`my-css-plugin` did no work in any of its 10012 calls, so this is the hook to fix. Two details are important:

- The time of 307.99 s is much larger than the build time of 843.6 ms. The column is the sum of all calls. Each call also includes the time that it waits for the JavaScript thread. Do not read this column as build time.
- `builtin:*` hooks run in Rust, so their no-op calls cost little.

### Check the result in `delta.md`

Add the filter. Then run the build again. See [Why Plugin Hook Filters?](/in-depth/why-plugin-hook-filter) for the reason that a filter helps.

```js
transform: {
  filter: { id: /\.css$/ },
  handler(code, id) {
    // ... transform CSS
  },
},
```

`delta.md` compares this build with the previous build:

| Metric                    | Previous | Current  | Δ         | Δ%     |
| ------------------------- | -------- | -------- | --------- | ------ |
| `build.scan_ms`           | 452.5 ms | 427.9 ms | -24.5 ms  | -5.4%  |
| `build.total_ms`          | 843.6 ms | 810.6 ms | -33.0 ms  | -3.9%  |
| `plugins.hook_call_count` | 190086   | 180074   | -10012    | -5.3%  |
| `plugins.hook_time_ms`    | 310.71 s | 2.64 s   | -308.07 s | -99.2% |

To measure the change, use `build.total_ms`, not `plugins.hook_time_ms`. The drop in hook time is 99%, but the hooks waited for most of that time.

One pair of builds also has noise. In 5 pairs of these builds, the drop was from 3.9% to 6.7%. To test a change, [pin a baseline](#compare-builds). Then run more than one build.

## How Rolldown writes the report

While the build runs, Rolldown converts its events into counts, sizes, graphs and timings in memory. It does not keep the source text of modules. When the bundle closes, Rolldown writes the files.

```dot
digraph {
    bgcolor="transparent";
    rankdir=LR;
    node [shape=box, style="filled,rounded", fontname="Arial", fontsize=12, margin="0.2,0.1", color="${#3c3c43|#dfdfd6}", fontcolor="${#3c3c43|#dfdfd6}"];
    edge [fontname="Arial", fontsize=10, color="${#3c3c43|#dfdfd6}", fontcolor="${#3c3c43|#dfdfd6}"];

    build [label="Build\nscan, generate", fillcolor="${#fff0e0|#4a2a0a}"];
    memory [label="Memory\ncounts, sizes,\ngraphs, timings", fillcolor="${#dbeafe|#1e3a5f}"];
    folder [label="Report folder\nentry.md, metrics.json,\n.state.json, ...", fillcolor="${#dcfce7|#14532d}"];
    next [label="Next build", fillcolor="${#fff0e0|#4a2a0a}"];

    build -> memory [label="events"];
    memory -> folder [label="bundle closes"];
    folder -> next [label="compare with\n.state.json", style=dashed];
}
```

- `metrics.json` has the numbers, and the Markdown files show parts of them.
- `module-graph.json` and `chunk-graph.json` have the complete graphs, which are too large for the Markdown lists.
- `.state.json` has the numbers of this build. Rolldown compares the next build with this file to write `delta.md`.

## Files

Start with `entry.md`. It summarizes the build and tells you which file to open next.

| File                | Contents                                                                                                                                |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `entry.md`          | Build time, output size, module, chunk and package counts, and the change since the previous build.                                     |
| `timing.md`         | Scan and generate times, plugin timings for each hook, the hooks that did no work, and the slowest modules in plugin `transform` hooks. |
| `chunks.md`         | The largest chunks, the reason each chunk exists, and modules that are in more than one chunk.                                          |
| `modules.md`        | Import kinds, the most-imported modules, and modules that several entries share.                                                        |
| `packages.md`       | The largest packages, direct and transitive dependencies, and packages with more than one version.                                      |
| `graph.md`          | Entry points, the chunk import graph, the initial load of each entry, and retained sizes.                                               |
| `delta.md`          | The change since the previous build and since the pinned baseline. Rolldown writes this file from the second build on.                  |
| `metrics.json`      | The numbers of the build, with stable metric ids such as `output.total_bytes` and `build.total_ms`. Use this file in scripts.           |
| `module-graph.json` | The complete module graph: every module with its size, its imports and its retained size.                                               |
| `chunk-graph.json`  | Every output chunk with its file name, size, entry flag and imports.                                                                    |
| `history.jsonl`     | One summary line per build.                                                                                                             |
| `AGENTS.md`         | Instructions for a coding agent that reads the folder.                                                                                  |
| `.state.json`       | The numbers of this build. Rolldown compares the next build with this file.                                                             |

## How to read the numbers

- `build.total_ms` includes the scan and generate stages. It is shorter than the time that the CLI shows.
- In the report, 1 kB is 1024 bytes. In the CLI, 1 kB is 1000 bytes. So the CLI can show `271.50 kB` for a chunk that the report shows as `265.1 kB`.
- Rolldown measures chunk and asset sizes before `generateBundle` hooks and before compression. These sizes do not show sourcemap files, or the files that `generateBundle` or `writeBundle` hooks add or edit.
- Module and package sizes include rendered module code after tree shaking, and before `renderChunk` hooks and chunk minification. If a module is in more than one chunk, the size includes each copy.
- The initial load of an entry is its chunk plus every chunk that it imports statically, directly or through other chunks.
- A direct package is a package that a module under `cwd` and outside `node_modules` imports. All other packages are transitive. Rolldown uses the import graph for this, not the dependency fields of `package.json`.
- Most lists keep the top 20 items. `module-graph.json` has every module, and `chunk-graph.json` has every chunk.
- Plugin timings include `resolveId`, `load`, `transform` and `renderChunk`. Each timing is the sum of calls that run at the same time, so it can be much larger than the build time. The `Transform hotspots` table in `timing.md` does not include the TypeScript and JSX transformation in Rolldown core.
- A no-op call is a hook call that did no work: `resolveId`, `load` or `renderChunk` returned nothing, or `transform` returned nothing or the same code. For a JavaScript plugin, each no-op call still crosses from Rust to JavaScript. A [hook filter](/apis/plugin-api/hook-filters) skips the call, and the report does not include it. Builtin plugins (`builtin:*`) run in Rust, so their no-op calls cost little. Before 1.3.0, the report shows only the No-op column of the hook table, and this column also includes the calls that the filter skips.

### Retained size

The retained size of a module is the size of the module plus the modules that only it brings in. It estimates the rendered module bytes that can leave the static initial load if you replace all eager imports of the module with `import()`. Rolldown calculates it across all user-defined entries. It does not predict compressed download savings or load time.

In this example, `a.js` and `b.js` both import `shared.js`, and only `a.js` imports `heavy.js`:

```dot
digraph {
    bgcolor="transparent";
    rankdir=TB;
    node [shape=box, style="filled,rounded", fontname="Arial", fontsize=12, margin="0.2,0.1", color="${#3c3c43|#dfdfd6}", fontcolor="${#3c3c43|#dfdfd6}"];
    edge [fontname="Arial", fontsize=10, color="${#3c3c43|#dfdfd6}", fontcolor="${#3c3c43|#dfdfd6}"];

    main [label="main.js", fillcolor="${#fff0e0|#4a2a0a}"];
    b [label="b.js\n5.0 kB", fillcolor="${#dbeafe|#1e3a5f}"];
    shared [label="shared.js\n50.0 kB", fillcolor="${#dbeafe|#1e3a5f}"];

    subgraph cluster_retained {
        label="Retained by a.js: 210.1 kB";
        labeljust="l";
        labelloc="b";
        fontname="Arial";
        fontsize=11;
        fontcolor="${#3c3c43|#dfdfd6}";
        style="dashed,rounded";
        color="${#d44803|#ff712a}";

        a [label="a.js\n10.0 kB", fillcolor="${#dcfce7|#14532d}"];
        heavy [label="heavy.js\n200.0 kB", fillcolor="${#dcfce7|#14532d}"];
    }

    main -> a;
    main -> b;
    a -> heavy;
    a -> shared;
    b -> shared;
}
```

`graph.md` shows the result:

| Module          | Own      | Retained     | Modules | via           |
| --------------- | -------- | ------------ | ------- | ------------- |
| `src/a.js`      | 10.0 kB  | **210.1 kB** | 2       | `src/main.js` |
| `src/heavy.js`  | 200.0 kB | **200.0 kB** | 1       | `src/a.js`    |
| `src/shared.js` | 50.0 kB  | **50.0 kB**  | 1       | `src/main.js` |
| `src/b.js`      | 5.0 kB   | **5.0 kB**   | 1       | `src/main.js` |

The retained size of `a.js` does not include `shared.js`, because `b.js` also imports it. `via` is the last module that every import path to this module goes through.

To check the number, replace the import of `a.js` in `main.js` with `import()`. The `Per-entry initial load` table in `delta.md` then shows the drop:

| Entry         | Previous | Current | Δ         | Δ%     |
| ------------- | -------- | ------- | --------- | ------ |
| `src/main.js` | 265.1 kB | 55.2 kB | -210.0 kB | -79.2% |

Read this table, not `output.max_initial_load_bytes`. That metric uses the largest entry, and `src/a.js` is now an entry with an initial load of 265.3 kB. To calculate the drop for several modules at once, use the [perf-kit commands](/in-depth/perf-kit#find-code-that-can-load-later).

## Compare builds

Rolldown compares each build with the previous build of the same output folder. To compare with one specific build, pin that build as the baseline:

```sh
$ cd node_modules/.rolldown/report/dist
$ cp .state.json baseline.json
```

Rolldown then also compares every later build with the baseline, in `delta.md` and in `metrics.json` (`baselineDelta`). To unpin the baseline, delete `baseline.json`.

## Use the numbers in scripts

`metrics.json` has a flat `metrics` map with stable ids. The id gives the unit: `_ms` for milliseconds, `_bytes` for bytes, and no suffix for counts. `AGENTS.md` in the report folder has every id.

```sh
$ cd node_modules/.rolldown/report/dist

# Read one number
$ jq '.metrics["output.total_bytes"]' metrics.json
1887396

# Fail a CI step when the output is too large (`-e` exits with 1 on false)
$ jq -e '.metrics["output.total_bytes"] < 2000000' metrics.json
true

# Show the JavaScript plugin hooks in the top no-op hooks
$ jq -r '.noopHooks[] | select(.plugin | startswith("builtin:") | not) | "\(.plugin) \(.hook) \(.noopCalls)/\(.calls)"' metrics.json
my-css-plugin transform 10012/10012
```

The other JSON data has these fields:

| Data                                                | Fields                                                                                                                                          |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `delta.metrics` and `baselineDelta.metrics`, per id | `prev`, `curr`, `delta`, `pct`                                                                                                                  |
| `modules` in `module-graph.json`                    | `id`, `bytes`, `retainedBytes`, `retainedModuleCount`, `idom`, `staticReachable`, `dynamicOnly`, `imports` (each is `[targetIndex, isDynamic]`) |
| `chunks` in `chunk-graph.json`                      | `fileName`, `name`, `bytes`, `isEntry`, `facadeModuleId`, `imports`, `dynamicImports`                                                           |

## Limitations

- [`watch()`](/reference/Function.watch), `rolldown --watch` and the dev engine (`experimental.devMode`) do not write a report.
- A build writes the report or the devtools log (`mode: 'full'`, the default). It does not write both.
- The report is inside `node_modules`, so a clean install deletes it, together with the pinned baseline.
- One bundle writes one report: when a bundle writes several outputs, the first output sets the folder, and the report describes the last output.

## Related tools

- The default `'full'` mode writes the complete devtools event log. Use this mode when you need every event in `@rolldown/debug` or Vite DevTools.
- [`bundleAnalyzerPlugin`](/builtin-plugins/bundle-analyzer) writes one analysis file of chunks and modules into the output folder. The report adds build and plugin timings, package data and the changes between builds. Rolldown writes the report outside the output folder.
- [`@rolldown/perf-kit`](/in-depth/perf-kit) measures page load, server startup and Cloudflare Worker startup for a coding agent that makes your app start faster. It reads `module-graph.json` to [find code that can load later](/in-depth/perf-kit#find-code-that-can-load-later), and it reads `chunk-graph.json` to [find chunks that start late](/in-depth/perf-kit#turn-on-the-build-report).
