# Perf Kit

`@rolldown/perf-kit` measures how fast a built app starts, and finds the code that can load later. It measures a page in the browser, a Node.js or Deno server, and a Cloudflare Worker.

perf-kit is made for a coding agent. Each command is one step of an optimization loop, and its output names the next step. The agent changes the code and decides what to keep. perf-kit measures each change against a pinned baseline, and it does not call the work done while a lead is still open.

:::tip EXPERIMENTAL
perf-kit is experimental, and its commands and output may change. The npm release 0.0.1 has the browser mode, the server mode and the rendered-output check. Worker mode, field mode, replay and the late chunk check need a later release, or the copy in the [Rolldown repository](https://github.com/rolldown/rolldown/tree/main/packages/perf-kit).
:::

## Requirements

- Node.js 22 or later.
- Chrome or Edge on the machine. `CHROME_PATH` selects the browser.
- A built app. perf-kit does not build it.
- For the [static commands](#static-analysis-with-the-build-report): a build with Rolldown 1.2.16 or later and the [build report](/in-depth/build-report) turned on.

perf-kit has no dependencies, so it runs without an install step:

```sh
$ npx @rolldown/perf-kit help
$ npx @rolldown/perf-kit scan --app .
```

To add it to the project instead, run `npm i -D @rolldown/perf-kit` and then `npx perf-kit scan --app .`. perf-kit keeps its state in `.perf-kit/` in the folder you run it from. It writes a `.gitignore` there, so the state stays out of commits.

## Give it to an agent

`help` prints the path of `AGENTS.md`, the manual for the agent. [`PROMPTS.md`](https://github.com/rolldown/rolldown/blob/main/packages/perf-kit/PROMPTS.md) has one prompt for each target. A prompt names the target, the number that counts, the rules and the report. It tells the agent to run `help` first:

```text
Make <page> of our app at <repo path> load faster. ...

Tools: run `npx @rolldown/perf-kit help` first. Its AGENTS.md is the manual.
```

The prompt does not tell the agent how to measure. The manual does that.

## The loop

1. Build the app and run `scan --app <dir>`. The first scan pins the baseline and saves a copy of the build as the original. Later commands remember the target.
2. Read the leads in the scan output. Each lead names a next step.
3. Change one thing in the app. Rebuild it and run the app's own tests.
4. Run `scan`. Keep the change only when it is faster beyond noise, and then run `scan --pin`. Otherwise, revert it.
5. When you are done, run `scan --parity` and `verdict`.

## What a scan measures

`scan` loads the built page in a headless Chrome with a throttled mobile profile: a 1.6 Mbit/s network with a 150 ms round trip, a 4x CPU slowdown and a cold cache. On a very large app, the first scan raises the bandwidth up to 8 times, until LCP fits under 8 s, and pins that scale. `scan --bandwidth <Mbit/s> --rtt <ms> --cpu <rate>` pins the network of a real environment instead.

One scan does 3 timed runs, one coverage run, and a CPU profile when the CPU work before the first paint is more than 150 ms. The scan then prints its leads. These are some of them:

| Lead                        | What it means                                                                                          |
| --------------------------- | ------------------------------------------------------------------------------------------------------ |
| Render-blocking CSS         | The last render-blocking stylesheet ends at FCP. CSS, not JavaScript, holds the first paint.           |
| Render gap                  | LCP comes after the load event.                                                                        |
| Late chunks                 | A static chunk starts more than one round trip after its ancestor chunk finished, before FCP or LCP.   |
| CPU before paint            | Sampled CPU time by source module, through the sourcemaps of every chunk.                              |
| Cold bytes at paint         | Bytes that load before the first paint, but mostly do not run before it.                               |
| Statically retained imports | A module outside the framework that retains 100 KB or more of the initial load, from the build report. |

The late chunk check reads `chunk-graph.json` from the build report, or a Vite manifest (`build.manifest: true`). Rolldown 1.2.16 does not write `chunk-graph.json`.

`verdict` combines all leads into one checklist, and marks each lead OPEN, clear or UNKNOWN. It does not report "done" while a lead is open, or while a signal is missing or older than the build.

## When a change counts

A change counts when FCP or LCP improves by at least max(30 ms, 2% of its baseline), and neither gets worse by that amount. Make this decision on a full scan (`scan --no-early-stop`). `scan --quick` does one run and is only a probe. The result is in `runtime-metrics.json`, in `paintComparison` and `baselineDelta`.

A change also counts only when the app's own tests pass. A faster app that lost a feature is a revert, not a gain.

## The rendered page must not change

A faster page that renders something else is not a gain. For example, a font that now never loads, or a hidden hero image, can make LCP measure a smaller element. The scan only tells you when the LCP element is not the original's. The full check runs once, at the end: `scan --parity` compares every pinned page with the original build. It compares the pixels of the first screen and of the full page, the page height, the visible text, the accessible names, new runtime errors and the LCP element.

Both builds render in one browser session. The original renders a second time with another clock and another random seed, and the areas that change between the 2 renders of the original are not compared. A difference starts recover mode: the numbers of that build do not count until it matches the original again. Only a person can accept a difference (`parity approve`) or add a mask, and every later verdict lists it.

The scan finds the HTML entry pages. Add client-side routes with `--pages /pricing,/settings`.

## Static analysis with the build report

These commands read `module-graph.json` from the [build report](/in-depth/build-report). They need no browser run:

| Command               | What it answers                                                                                                                |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `graph`               | Modules ranked by retained size: the bytes that leave the initial load when one import becomes dynamic, with the chain to cut. |
| `what-if <module>...` | The exact modules and bytes that a deferral frees. With 2 or more modules, it prices them as one plan.                         |
| `cut <module>`        | The fewest import statements to make dynamic, when several paths reach the module.                                             |
| `why <module>`        | The direct static importers of the module, and one import chain from the entry.                                                |
| `graph-diff`          | The modules that entered or left the initial load since the pinned baseline, with their bytes.                                 |

Static numbers are leads, not measurements. Check the effect of a change with `scan`. The analysis starts at the build entries, so a dynamic import that runs on every start is not part of the initial load. Pass the modules that it loads with `--root` to `what-if` and `cut`.

## Other targets

### Node.js and Deno servers

```sh
$ npx @rolldown/perf-kit node-scan --entry dist/server.js --ready port:3000
```

`--ready` declares when the server is up: `port:<n>`, `stdout:<regex>`, an `http://` URL that answers 2xx, or `exit`. perf-kit pins it with the target. The scan times the runs from start to ready. Separate runs record what had to run before ready and where the CPU time went. `runtime.app_startup_ms` subtracts the time that an empty script takes on the machine. `--exec deno` runs the server with Deno. Bun is not supported.

### Cloudflare Workers

```sh
$ npx @rolldown/perf-kit worker-config --app <dir>
$ (cd <dir> && npx vite build --config perf-kit.vite.config.ts)
$ npx @rolldown/perf-kit worker-scan --app <dir> --request /
```

`worker-scan` measures the global scope of the Worker under the app's own workerd. That is the scope that the Cloudflare startup limit counts. The scan attributes the time to modules, and finds imports that only handlers or `async` functions read. Each scan also times the pinned build next to the current build, in alternating runs, so that machine load moves both alike.

The app must build with `@cloudflare/vite-plugin` and have `wrangler` as a dev dependency. Coverage needs Wrangler 4.108.0 or later. The numbers come from the local CPU. Use `wrangler check startup`, before and after, for the number that Cloudflare reports.

### A deployed site

`field --config <file>` measures the deployed site in your own logged-in Chrome. It loads the variants in alternating pairs and splits LCP along the request that LCP waited for. `record` saves one deployed variant, and `replay` serves that recording to a headless Chrome with a local build in place of the deployed files. This gives field-like numbers for a build that is not deployed. The output and the recordings hold real responses from your account, so perf-kit writes them outside the repository.

## Limits

- Lab numbers are valid only on one machine and with the pinned throttle. The signal is the difference between two builds.
- V8 coverage counts the top level of a module as run when the module loads. A module that holds its data at the top level looks used at paint.
- The rendered-output check sees the page as it is served, without interaction. States behind a click, a login or live data are not compared.
- perf-kit does not collect metrics from real users. Field mode measures one account, one network and one machine.

## Related

- The [build report](/in-depth/build-report) gives the module graph and the chunk graph that the static commands and the late chunk check read.
- The perf-kit [README](https://github.com/rolldown/rolldown/blob/main/packages/perf-kit/README.md) lists every command, flag and output file.
