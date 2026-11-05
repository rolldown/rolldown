# Build Report

With `devtools: { mode: 'report' }`, Rolldown writes a build report: a folder of small Markdown and JSON files about each build. The report shows where the build time goes, what the output contains, and what changed since the previous build. Give the folder to a coding agent, read it yourself, or use its numbers in scripts.

::: warning Experimental
The `devtools` option is experimental, and the report format may change in future releases. The report needs Rolldown 1.2.16 or later.
:::

## Why a report

The default `'full'` mode records every build event, with the source text of each module, for visual tools such as Vite DevTools. A coding agent needs no visual view. With only this log, it has to write its own scripts to extract numbers from a file that can reach several gigabytes on a large app, and that work uses tokens and space in its context window.

The report is designed for coding agents. Rolldown computes the numbers, such as plugin timings, build changes and retained sizes, and the agent reads the result. An agent starts with `entry.md` (1.6 KB in the build below) and opens other files only when it needs them. People and scripts can use the report too.

This table shows one build of [`apps/10000`](https://github.com/rolldown/benchmarks/tree/main/apps/10000) from the benchmark repository:

| `devtools`       | Data written | Build time |
| ---------------- | ------------ | ---------- |
| Off              | None         | 1.18 s     |
| `mode: 'report'` | 4.5 MB       | 1.59 s     |
| `mode: 'full'`   | 185 MB       | 1.59 s     |

Build time is the median of 5 runs on an Apple M3 Max, with Node.js startup included. Both modes collect the same events, so both make the build slower. Turn the report on when you investigate a build, not for every build.

Most of the 4.5 MB is `module-graph.json`, which agents and scripts query and do not read whole. All the Markdown files together are 14 KB, because most lists keep only the top 20 items.

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

Each output folder keeps its own report, so a build compares against the previous build of the same output.

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

## Read a report

This example adds one plugin to `apps/10000`. The plugin handles only `.css` files, but it has no [hook filter](/apis/plugin-api/hook-filters), so Rolldown calls it for every module. The output comes from the code for Rolldown 1.3.0. Rolldown 1.2.16 prints less.

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

The summary gives the main numbers and names the file to open next:

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

`my-css-plugin` did no work in any of its 10012 calls, so it is the hook to fix. Two details are important:

- The time of 307.99 s is much larger than the 843.6 ms build. The column adds up all calls, and each call also counts the time that it waits for the JavaScript thread. Do not read it as build time.
- `builtin:*` hooks run in Rust, so their no-op calls cost little.

### Check the fix in `delta.md`

Add the filter and build again. See [Why Plugin Hook Filters?](/in-depth/why-plugin-hook-filter) for the reason that a filter helps.

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

Judge the change by `build.total_ms`, not by `plugins.hook_time_ms`. The hook time dropped by 99%, but most of it was waiting.

One pair of builds also has noise: 5 pairs of these builds gave drops from 3.9% to 6.7%. To test a change, [pin a baseline](#compare-builds) and build more than once.

## How it works

While the build runs, Rolldown folds its events into counts, sizes, graphs and timings in memory. It does not keep the source text of modules. When the bundle closes, Rolldown writes the files.

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
    memory -> folder [label="bundle close"];
    folder -> next [label="compare with\n.state.json", style=dashed];
}
```

- `metrics.json` holds the numbers, and the Markdown files show parts of them.
- `module-graph.json` and `chunk-graph.json` hold the complete graphs, which are too large for the Markdown lists.
- `.state.json` saves the numbers of this build. The next build compares against it to write `delta.md`.

::: info Why many small files?
A person or a coding agent usually needs one part of the report. With small files, they open only that part, and scripts get the exact numbers from `metrics.json`.
:::

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
| `chunk-graph.json`  | Every output chunk with its file name, size, entry flag and imports.                                                                                                             |
| `history.jsonl`     | One summary line per build.                                                                                                                                                      |
| `AGENTS.md`         | Instructions for a coding agent that reads the folder.                                                                                                                           |
| `.state.json`       | A snapshot of this build. The next build compares against it.                                                                                                                    |

## How to read the numbers

- `build.total_ms` covers the scan and generate stages. It is shorter than the time that the CLI prints.
- The report counts 1 kB as 1024 bytes, and the CLI counts 1 kB as 1000 bytes. So the CLI can show `271.50 kB` for a chunk that the report shows as `265.1 kB`.
- Chunk and asset sizes are measured before `generateBundle` hooks, before compression, and exclude sourcemap files. Files added or changed by `generateBundle` or `writeBundle` hooks are not reflected in these sizes.
- Module and package sizes count rendered module code after tree shaking, before `renderChunk` hooks and chunk minification. A module duplicated across chunks is counted once per copy.
- The initial load of an entry is its chunk plus every chunk that it imports statically, directly or through other chunks.
- Direct packages are those imported by project modules under `cwd` and outside `node_modules`; other packages are classified as transitive. This classification uses the import graph, not `package.json` dependency fields.
- Most lists keep the top 20 items. `module-graph.json` lists every module, and `chunk-graph.json` every chunk.
- Plugin timings cover `resolveId`, `load`, `transform` and `renderChunk`. They add up calls that run at the same time, so they can be much larger than the build time. Core TypeScript and JSX transformation is not attributed per module in the transform hotspots.
- A hook call did no work when `resolveId`, `load` or `renderChunk` returned nothing, or `transform` returned nothing or the code unchanged. For a JavaScript plugin, each such call still crosses from Rust to JavaScript. A [hook filter](/apis/plugin-api/hook-filters) skips the call, and a skipped call does not count. Builtin plugins (`builtin:*`) run in Rust, so their no-op calls cost little. Before 1.3.0, the report shows only the No-op column of the hook table, and it also counts the calls that the filter skips.

### Retained size

The retained size of a module estimates the rendered module bytes that could leave the static initial load if all eager import paths to it were deferred: the module itself plus the modules that only it brings in. It is calculated across all user-defined entries, and is not a prediction of compressed download savings or load time.

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

To check the number, change the import of `a.js` in `main.js` to `import()`. The `Per-entry initial load` table in `delta.md` then shows the drop:

| Entry         | Previous | Current | Δ         | Δ%     |
| ------------- | -------- | ------- | --------- | ------ |
| `src/main.js` | 265.1 kB | 55.2 kB | -210.0 kB | -79.2% |

Read this table, not `output.max_initial_load_bytes`. That metric takes the largest entry, and `src/a.js` is now an entry with an initial load of 265.3 kB. To price several modules at once, use the [perf-kit commands](/in-depth/perf-kit#find-code-that-can-load-later).

## Compare builds

Each build compares against the previous build of the same output folder. To compare against a fixed build, pin it as the baseline:

```sh
$ cd node_modules/.rolldown/report/dist
$ cp .state.json baseline.json
```

Every later build then also compares against the baseline, in `delta.md` and in `metrics.json` (`baselineDelta`). Delete `baseline.json` to unpin it.

## Use the numbers in scripts

`metrics.json` has a flat `metrics` map with stable ids. The id gives the unit: `_ms` for milliseconds, `_bytes` for bytes, and no suffix for counts. `AGENTS.md` in the report folder lists every id.

```sh
$ cd node_modules/.rolldown/report/dist

# Read one number
$ jq '.metrics["output.total_bytes"]' metrics.json
1887396

# Fail a CI step when the output is too large (`-e` exits with 1 on false)
$ jq -e '.metrics["output.total_bytes"] < 2000000' metrics.json
true

# List the JavaScript plugin hooks among the top no-op hooks
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

- Only one-off builds write a report. [`watch()`](/reference/Function.watch), `rolldown --watch` and the dev engine (`experimental.devMode`) write none.
- A build writes the report or the full devtools log (`mode: 'full'`, the default), not both.
- The report is inside `node_modules`, so a clean install deletes it, together with the pinned baseline.
- One bundle writes one report: when a bundle writes several outputs, the first output picks the folder, and the report describes the last output.

## Related tools

- The default `'full'` mode writes the complete devtools event log. Use it when you need every event, in `@rolldown/debug` or Vite DevTools.
- [`bundleAnalyzerPlugin`](/builtin-plugins/bundle-analyzer) writes one chunk and module analysis file into the output folder. The report adds build and plugin timings, package data and build-over-build changes, and it stays out of the output folder.
- [`@rolldown/perf-kit`](/in-depth/perf-kit) measures page load, server startup and Cloudflare Worker startup for a coding agent that makes your app start faster. It reads `module-graph.json` to [find code that can load later](/in-depth/perf-kit#find-code-that-can-load-later), and `chunk-graph.json` to [find chunks that start late](/in-depth/perf-kit#turn-on-the-build-report).
