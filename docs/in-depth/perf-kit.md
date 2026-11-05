# Perf Kit

`@rolldown/perf-kit` helps a coding agent make your app start faster. It measures a page in the browser, a Node.js or Deno server, or a Cloudflare Worker. The agent changes the code, and perf-kit shows which changes to keep.

:::tip EXPERIMENTAL
perf-kit is experimental, and its commands and output may change. The npm release 0.0.1 has the browser mode, the server mode and the rendered-output check. Worker mode, field mode, replay and the late chunk check need a later release, or the copy in the [Rolldown repository](https://github.com/rolldown/rolldown/tree/main/packages/perf-kit).
:::

## Setup

You need:

- Node.js 22 or later.
- A built app. perf-kit does not build it.
- For a page: Chrome or Edge on the machine. `CHROME_PATH` selects the browser.
- For a Worker: `@cloudflare/vite-plugin` and `wrangler` as dev dependencies of the app. Coverage needs Wrangler 4.108.0 or later.

perf-kit has no dependencies, so it runs without an install step:

```sh
$ npx @rolldown/perf-kit help
```

To add it to the project instead, run `npm i -D @rolldown/perf-kit`. The command is then `npx perf-kit`. perf-kit keeps its state in `.perf-kit/` in the folder you run it from. It writes a `.gitignore` there, so the state stays out of commits.

### Turn on the build report

This step is optional. If the app builds with Rolldown 1.2.16 or later, turn on the [build report](/tools/build-report#usage) and rebuild the app. perf-kit then reads 2 of its files:

- `module-graph.json`, for the commands that [find code that can load later](#find-code-that-can-load-later).
- `chunk-graph.json`, for the check that finds chunks that start late. Rolldown 1.2.16 does not write this file. A Vite manifest (`build.manifest: true`) also works.

## Use it with a coding agent

1. Copy the prompt for your target.
2. Fill in the `<...>` parts. Keep the other text as it is.
3. Give the prompt to the agent.

The prompt tells the agent to run `help` first. `help` prints the path of `AGENTS.md`, the manual for the agent. Do not add steps that tell the agent how to measure or profile. The manual does that, and a prompt that also does it moves the agent away from the manual.

### Browser page load

```text
Make <page> of our app at <repo path> load faster. Dependencies are installed
with <pm>. The build command is `<build command>`.

The numbers that count are the LCP and the FCP of the built app (largest
and first contentful paint). Measure them on a throttled mobile profile: fast
3G, 4x CPU, cold cache. Take a baseline before any change. Report every
result against it. Runs are noisy. You decide the run count.

Tools: run `npx @rolldown/perf-kit help` first. Its AGENTS.md is the manual.

Rules:
- Do not remove features.
- The rendered page must not change.
- `<test command>` must stay green.
- Make one change at a time. Keep it only when it is clearly faster. Revert
  all other changes.

Report:
- the numbers before and after
- every change you tried, with its delta, the reverted ones too
- for each kept change, the file and why it was safe
```

### Node or Deno server startup

```text
Cut the startup time of our server at <repo path>. Dependencies are installed
with <pm>. The build command is `<build command>`. The entry is
`<dist/server.js>`.

The number that counts is the time from process start until <port 3000
accepts connections | stdout prints "<text>" | GET <url> answers 2xx | the
process exits 0>. Use a cold compile cache. Take a baseline before any
change. Report every result against it. Runs are noisy. You decide the run
count.

Tools: run `npx @rolldown/perf-kit help` first. Its AGENTS.md is the manual.
Read its server section.

Rules:
- Do not remove features.
- Keep the readiness signal as declared.
- `<test command>` must stay green.
- Make one change at a time. Keep it only when it is clearly faster. Revert
  all other changes.

Report:
- the numbers before and after
- every change you tried, with its delta, the reverted ones too
- for each kept change, the file and why it was safe
```

### Cloudflare Worker startup

perf-kit 0.0.1 does not have the Worker commands. Clone the [Rolldown repository](https://github.com/rolldown/rolldown), and replace `<perf-kit path>` with the absolute path of `packages/perf-kit`.

```text
Cut the cold start of our Cloudflare Worker at <repo path>. Dependencies are
installed with <pm>.

The number that counts is `wrangler check startup` on the built Worker. Take
a baseline before any change. Report every result against it. Runs are
noisy. You decide the run count.

Tools: run `node <perf-kit path>/harness.mjs help` first. Its AGENTS.md is the
manual. Read its Cloudflare Worker section.

Rules:
- Do not remove features.
- Do not change bindings or the wrangler config.
- Keep each Durable Object class exported from the entry.
- `<test commands>` must stay green.
- Make one change at a time. Keep it only when it is clearly faster. Revert
  all other changes.

Report:
- the numbers before and after
- every change you tried, with its delta, the reverted ones too
- for each kept change, the file and why it was safe
```

## Read the result

The agent reports the numbers before and after, and every change that it tried. For a page, also check these 2 items:

- **The verdict checklist.** It marks each lead OPEN, clear or UNKNOWN. For each OPEN lead, the agent must give a reason: a measurement or a constraint.
- **The rendered-output check.** `scan --parity` compares the final build with the original. If they differ, the numbers of that build do not count until it matches again. Only you can accept a difference (`parity approve`) or add a mask, and every later verdict lists it.

## Run the commands yourself

You can also run the commands without an agent. The [README](https://github.com/rolldown/rolldown/blob/main/packages/perf-kit/README.md) lists every command, flag and output file.

### A page

Run each command as `npx @rolldown/perf-kit <command>`.

1. Build the app and run `scan --app <dir>`. The first scan pins the baseline and saves a copy of the build as the original. Later commands remember the target.
2. Read the leads in the output. Each lead names a next step.
3. Change one thing. Rebuild the app and run its tests.
4. Run `scan --no-early-stop`. If it reports an improvement beyond noise, run `scan --pin`. Otherwise, revert the change.
5. When you are done, run `scan --parity` and `verdict`.

These `scan` flags are often useful:

| Flag                                           | Use                                                                                                                         |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `--dist <dir>`                                 | Selects the build output. Without it, `--app` looks for `dist`, `build` and `out`.                                          |
| `--pages /pricing,/settings`                   | Adds client-side routes. The scan finds the HTML entry pages without this flag.                                             |
| `--quick`                                      | Does one run. Use it as a probe, not to decide if you keep a change.                                                        |
| `--bandwidth <Mbit/s> --rtt <ms> --cpu <rate>` | Pins the network of a real environment. The default is a 1.6 Mbit/s network with a 150 ms round trip and a 4x CPU slowdown. |

### A Node.js or Deno server

```sh
$ npx @rolldown/perf-kit node-scan --entry dist/server.js --ready port:3000
```

`--ready` declares when the server is up: `port:<n>`, `stdout:<regex>`, an `http://` URL that answers 2xx, or `exit`. perf-kit pins it with the target. `--ready exit` and a `stdout:` pattern that starts with `^` time out under the inspector at this time.

`--exec deno` runs the server with Deno. Bun is not supported. After a kept change, run `node-scan --pin`.

### A Cloudflare Worker

```sh
$ node <perf-kit path>/harness.mjs worker-config --app <dir>
$ (cd <dir> && npx vite build --config perf-kit.vite.config.ts)
$ node <perf-kit path>/harness.mjs worker-scan --app <dir> --request /
```

`worker-config` writes a Vite config that wraps the config of the app. `--request` adds a route to call after startup. After a kept change, run `worker-scan --pin`. The numbers come from the local CPU. Use `wrangler check startup`, before and after, for the number that Cloudflare reports.

### A deployed site

`field --config <file>` measures the deployed site in your own logged-in Chrome. `record` saves one deployed variant. `replay` serves that recording to a headless Chrome with a local build in place of the deployed files. The output and the recordings hold real responses from your account, so perf-kit writes them outside the repository.

### Find code that can load later

These commands need the [build report](#turn-on-the-build-report), and they do not need a browser run. Their numbers are leads, not measurements, so check each change with `scan`.

| Command               | What it answers                                                                                                                |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `graph`               | Modules ranked by retained size: the bytes that leave the initial load when one import becomes dynamic, with the chain to cut. |
| `what-if <module>...` | The exact modules and bytes that a deferral frees. With 2 or more modules, it prices them as one plan.                         |
| `cut <module>`        | The fewest import statements to make dynamic, when several paths reach the module.                                             |
| `why <module>`        | The direct static importers of the module, and one import chain from the entry.                                                |
| `graph-diff`          | The modules that entered or left the initial load since the pinned baseline, with their bytes.                                 |

## Limits

- Lab numbers are valid only on one machine and with the pinned throttle. The signal is the difference between two builds.
- The rendered-output check sees the page as it is served, without interaction. States behind a click, a login or live data are not compared, so keep the app's own tests green.
- perf-kit does not collect metrics from real users. Field mode measures one account, one network and one machine.
