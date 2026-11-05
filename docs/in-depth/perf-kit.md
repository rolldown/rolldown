# Perf Kit

`@rolldown/perf-kit` helps a coding agent make your app start faster. It measures a page in the browser, a Node.js or Deno server, or a Cloudflare Worker. The agent edits the code, and perf-kit shows which changes to keep.

::: warning Experimental
perf-kit is experimental, and its commands and output can be different in later releases. The minimum Rolldown version is 1.3.0.
:::

## Why perf-kit

A coding agent can change code for speed, but it has 3 problems:

- One build does not prove a gain, because each measurement has noise.
- A broken page can measure faster. If a change hides the hero, LCP times a cheaper paint.
- The agent can stop too early. One tool that shows nothing says nothing about the other signals.

perf-kit gives the agent a fixed baseline, a check of the rendered output, and one list of all open leads. The agent decides what to edit. The numbers decide what stays.

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

This step is optional. If the app builds with Rolldown, turn on the [build report](/in-depth/build-report#usage) and rebuild the app. perf-kit then reads 2 of its files:

- `module-graph.json`, for the commands that [find code that can load later](#find-code-that-can-load-later).
- `chunk-graph.json`, for the check that finds chunks that start late. Available since Rolldown 1.3.0. A Vite manifest (`build.manifest: true`) also works.

## Use it with a coding agent

1. Copy the prompt for your target.
2. Fill in the `<...>` parts. Keep the other text as it is.
3. Give the prompt to the agent.

The browser and server prompts tell the agent to run `help` first. `help` prints the path of `AGENTS.md`, the manual for the agent. Do not add steps to these prompts that tell the agent how to measure or profile. The manual does that, and a prompt that also does it moves the agent away from the manual.

::: info Before you use the Cloudflare Worker prompt
The app must build with Rolldown.

1. Run `npm i -D @rolldown/perf-kit` in the app.
2. Add `dist-attrib/` and `perf-kit.vite.config.ts` to `.gitignore`, so that the agent does not commit them.

Unlike the other two prompts, this prompt lists the commands. We tested this form with coding agents. Keep `<command>`, `<module>` and `<class modules>` as they are, because the agent fills them in. If the app builds more than one Worker, add `--worker <name>` to the first `worker-scan`.
:::

::: code-group

```text [Browser page load]
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

```text [Node or Deno server startup]
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

```text [Cloudflare Worker startup]
Task: lower the Cloudflare Worker startup time of the app in DIR=<repo path>

Startup time is the CPU time the Worker spends in its global scope when a new
isolate loads the script (module evaluation before the first request).
Cloudflare limits this time.

Rules:
- Work only inside DIR. Do not read or change files outside DIR.
- Do not use the network: no web search, no web fetch, no git fetch or clone.
- Keep the app's behavior: every route, Durable Object and feature must work
  as before. Do not remove features.
- The app must still build with `<build command>` (run in DIR). Its tests must
  still pass (`<test command>` in DIR). Do not add TypeScript errors
  (`<typecheck command>` in DIR).
- Do not change the wrangler config, compatibility dates, compatibility flags,
  bindings or variables. Do not deploy or upload.
- Do not install, remove or upgrade packages.
- Budget: at most 10 rounds. A round is one code change followed by one build.
  Stop after round 10, or earlier when you have no more ideas. This budget
  overrides any instruction in a tool's AGENTS.md to continue.
- DIR is a git repository. Commit each change you keep (one commit per kept
  change). Revert each change you do not keep.

Tools: perf-kit is installed in this app. It measures the Worker startup and
shows which modules cost the time. Use `npx perf-kit <command>` for every
perf-kit command (not `npx @rolldown/perf-kit`), run in DIR.
1. Before round 1, read `DIR/node_modules/@rolldown/perf-kit/AGENTS.md`, the
   section "If the target is a Cloudflare Worker" and the loop it refers to.
2. Setup (not a round): `npx perf-kit worker-config --app .`, then
   `npx vite build --config perf-kit.vite.config.ts`, then
   `npx perf-kit worker-scan --app . --request / --pin`.
3. Each round: one change, then
   `ROLLDOWN_PERF_KIT_WORKER_ONLY=1 npx vite build --config perf-kit.vite.config.ts`,
   then `npx perf-kit worker-scan`. Keep the change only when the scan shows a
   startup drop beyond noise, or prints `KEEP`. `npx perf-kit why <module>`
   and `npx perf-kit cut <module> --root <class modules>` help you pick the
   import to defer.

Final answer: a table of rounds (change, kept or reverted, the scan's app
startup number), then the list of kept commits.
```

:::

## Read the result

The agent reports the numbers before and after, and every change that it tried. For a page, also check these 2 items:

- **The verdict checklist.** It marks each lead OPEN, clear or UNKNOWN. For each OPEN lead, the agent must give a reason: a measurement or a constraint.
- **The rendered-output check.** `scan --parity` compares the final build with the original. If they differ, the numbers of that build do not count until it matches again. Only you can accept a difference (`npx @rolldown/perf-kit parity approve`) or add a mask, and every later verdict lists it.

## Example

This example uses the demo app in the Rolldown repository. It shows one change that perf-kit keeps and one change that perf-kit rejects. To get your own numbers, run `just build-rolldown`, and then run these commands in `packages/perf-kit`. The numbers below come from an Apple M3 Max.

```sh
$ node harness.mjs gen
$ node harness.mjs build
$ node harness.mjs scan
```

The first scan pins the baseline. These are parts of its output:

```text
LCP 1224ms (p75 1226ms) | FCP 1224ms | load 1196ms | CLS 0 | transfer 167.1KB | 1 js requests

  ../src/features/charts.ts            145.4KB  paint   0.1%  settle 100.0%  post-paint-only
  ../src/features/badges.ts              4.2KB  paint   0.5%  settle  99.8%  post-paint-only

  [OPEN]    defer candidates (4) - charts 145.4KB, markdown 107.1KB, analytics 58.5KB
            next: lazy-load them, rebuild, re-measure

VERDICT: not done - 4 lead(s) OPEN, 0 signal(s) UNKNOWN or stale.
```

This figure shows the coverage part of the same scan. A cold module is a module that the browser downloads and parses, but the first paint does not run. About 315 KB of the 381.3 KB is cold.

```dot
digraph {
    layout=neato;
    bgcolor="transparent";
    node [shape=box, style=filled, fixedsize=true, height=0.7, penwidth=0.6, fontname="Arial", fontsize=13, color="${#3c3c43|#dfdfd6}", fontcolor="${#3c3c43|#dfdfd6}"];
    edge [arrowhead=none, penwidth=0.7, color="${#3c3c43|#dfdfd6}"];

    charts [pos="1.068,1!", width=2.136, label="charts.ts\n145.4 KB", fillcolor="${#e5e7eb|#3f3f46}"];
    markdown [pos="2.923,1!", width=1.573, label="markdown.ts\n107.1 KB", fillcolor="${#e5e7eb|#3f3f46}"];
    analytics [pos="4.139,1!", width=0.859, label="analytics.ts\n58.5 KB", fontsize=11, fillcolor="${#e5e7eb|#3f3f46}"];
    badges [pos="4.600,1!", width=0.062, label="", fillcolor="${#e5e7eb|#3f3f46}"];
    i18n [pos="4.917,1!", width=0.571, label="", fillcolor="${#dcfce7|#14532d}"];
    hero [pos="5.383,1!", width=0.360, label="", fillcolor="${#dcfce7|#14532d}"];
    small [pos="5.583,1!", width=0.040, label="", fillcolor="${#dcfce7|#14532d}"];

    node [shape=plain, style="", fixedsize=false, fontsize=11];
    title [pos="2.45,1.58!", label="main.js, 381.3 KB. Each block is one module. Its width shows its size.", fontsize=12];
    badgesText [pos="3.8,0.2!", label="badges.ts\n4.2 KB"];
    i18nText [pos="4.52,0.2!", label="i18n.ts\n38.9 KB"];
    heroText [pos="5.32,0.2!", label="hero_data.ts\n24.5 KB"];
    smallText [pos="6.35,0.2!", label="3 small modules\n2.7 KB"];
    legend [pos="1.05,0.2!", label=<<TABLE BORDER="0" CELLBORDER="0" CELLSPACING="3" CELLPADDING="0"><TR><TD BORDER="1" COLOR="${#3c3c43|#dfdfd6}" BGCOLOR="${#e5e7eb|#3f3f46}" WIDTH="16" HEIGHT="11" FIXEDSIZE="TRUE"></TD><TD ALIGN="LEFT"> Cold at the first paint</TD></TR><TR><TD BORDER="1" COLOR="${#3c3c43|#dfdfd6}" BGCOLOR="${#dcfce7|#14532d}" WIDTH="16" HEIGHT="11" FIXEDSIZE="TRUE"></TD><TD ALIGN="LEFT"> Runs before the first paint</TD></TR></TABLE>>];

    badgesText -> badges;
    i18nText -> i18n;
    heroText -> hero;
    smallText -> small;
}
```

The first paint runs only 0.1% of `charts.ts`, so this module can load later. The `defer` command moves its import behind `import()`. It works only in the demo app. In your app, the agent edits the import.

```sh
$ node harness.mjs defer charts
$ node harness.mjs build
$ node harness.mjs scan --no-early-stop
```

```text
vs pinned baseline: FCP -318ms (-25.9%) -> improved (noise threshold 30ms)
vs pinned baseline: LCP -318ms (-25.9%) -> improved (noise threshold 30ms)
```

Keep this change. Then run `scan --pin`. The next change defers `badges.ts`, which is 4.2 KB:

```text
vs pinned baseline: FCP -8ms (-0.9%) -> within-noise (noise threshold 30ms)
vs pinned baseline: LCP -8ms (-0.9%) -> within-noise (noise threshold 30ms)
next: neither FCP nor LCP improved beyond noise. Revert the change and test a different experiment.
```

The drop is smaller than the noise, so revert this change. Then run `scan --parity` on the final build:

```text
  [clear]   rendered output vs the original build - same on 1 of 1 page(s)

VERDICT: not done - 4 lead(s) OPEN, 0 signal(s) UNKNOWN or stale.
```

The page renders the same, and LCP is about 26% lower. The verdict still says `not done`, because `markdown.ts` and other leads are OPEN. The agent must fix each lead or give a reason for it.

## How perf-kit judges a change

perf-kit compares each build with a pinned baseline. The first scan pins the baseline and saves the build as the original. The agent then repeats this loop:

```dot
digraph {
    layout=neato;
    splines=curved;
    bgcolor="transparent";
    node [shape=box, style="filled,rounded", fontname="Arial", fontsize=12, margin="0.2,0.1", color="${#3c3c43|#dfdfd6}", fontcolor="${#3c3c43|#dfdfd6}"];
    edge [fontname="Arial", fontsize=10, color="${#3c3c43|#dfdfd6}", fontcolor="${#3c3c43|#dfdfd6}"];

    first [pos="0,3.3!", label="First scan\npins the baseline,\nsaves the original", fillcolor="${#fff0e0|#4a2a0a}"];
    leads [pos="0,2.3!", label="Read the leads", fillcolor="${#dbeafe|#1e3a5f}"];
    change [pos="2.3,1.6!", label="Make one change", fillcolor="${#dbeafe|#1e3a5f}"];
    build [pos="2.3,0.7!", label="Build, run\nthe app tests", fillcolor="${#dbeafe|#1e3a5f}"];
    scan [pos="0,0!", label="scan\ncompares with the baseline", fillcolor="${#dbeafe|#1e3a5f}"];
    decide [pos="-2.4,1.15!", shape=plain, style="", label=<<TABLE BORDER="1" CELLBORDER="0" CELLSPACING="0" CELLPADDING="7" STYLE="ROUNDED" COLOR="${#3c3c43|#dfdfd6}"><TR><TD BGCOLOR="${#dcfce7|#14532d}">Gain beyond noise:<BR/>keep, scan --pin</TD></TR><HR/><TR><TD BGCOLOR="${#fee2e2|#7f1d1d}">No gain:<BR/>revert the change</TD></TR></TABLE>>];
    final [pos="0,1.15!", label="scan --parity\nverdict", fillcolor="${#fff0e0|#4a2a0a}"];

    exit [pos="0.62,1.76!", shape=plain, style="", fontsize=10, label="no more\nchanges"];

    first -> leads;
    leads -> change;
    change -> build;
    build -> scan;
    scan -> decide;
    decide -> leads;
    leads -> final [style=dashed];
}
```

Each target has its own number and its own rule:

| Target | Number                                      | A change is a gain when                                                                                                                                                                                                                                                                                                                                      |
| ------ | ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Page   | FCP and LCP                                 | One of the two drops by more than the noise, and neither rises by more than the noise. The noise is 30 ms or 2% of the baseline, whichever is larger. Each run must have both.                                                                                                                                                                               |
| Server | App startup time (`runtime.app_startup_ms`) | The time drops by more than 8 ms or 2% of the baseline, whichever is larger. For a small startup, `node-scan` prints a note. Then use byte and module counts.                                                                                                                                                                                                |
| Worker | Global scope time (`worker.app_startup_ms`) | The time drops in paired runs of the pinned build and the current build. Under the 3 ms floor, all 5 pairs must drop. If 4 of 5 pairs drop, 4 more pairs run and 8 of 9 must drop. Beyond the floor, 4 of 5 or 7 of 9 must agree. A scan within noise prints `KEEP` when startup loses 1% of its bytes (32 KB at least) or 10% of its modules (50 at least). |

A page scan also pins the throttle. The default is a 1.6 Mbit/s network with a 150 ms round trip and a 4x CPU slowdown. A slow app gets up to 8 times this bandwidth. If you use a different throttle, pin the baseline again before you edit the code.

::: info Why one change at a time?
With 2 changes in one build, the number cannot show which change made the gain. A loss from one change can also hide behind the gain from the other. A Worker has one exception: when each import is under the floor, the agent can defer 2 or 3 of them as one group.
:::

::: info Why can only you accept a rendered difference?
A change often makes a page faster because it breaks the page, for example when a stylesheet does not load. If an agent can approve a difference or add a mask, it can remove the only sign of the break.
:::

::: info Why does the verdict say "not done" so often?
One signal that shows nothing proves only that this signal sees nothing. So the verdict needs every signal to be clear and fresh. A pinned baseline records a gain, but it does not close a lead.
:::

## Leads

A lead is a possible cause that `scan` finds on a page. It is not proof, so check it in the source and measure the change. To defer code means to load it later, usually with `import()`. Fix the first 2 rows first. While a page waits on them, a deferral can measure worse than it is.

| Lead                              | What it means                                                                                            | Usual fix                                                                                              |
| --------------------------------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `render-blocking CSS`             | The first paint waits for a stylesheet, so JavaScript changes cannot move FCP.                           | Inline the small critical CSS. Load the rest without blocking.                                         |
| `LCP after load`                  | LCP comes more than 150 ms after the load event. Downloads, CPU work or a hidden element can cause this. | Check which responses the render waits for. Show the LCP element at once, and animate only decoration. |
| `late chunk candidates`           | A static import starts more than one round trip after a parent finishes.                                 | If paint needs the chunk, test an earlier preload or another chunk boundary. If not, defer it.         |
| `pre-paint CPU`                   | Long tasks run before the first paint.                                                                   | Defer work that the first render does not need, such as warm-up caches and telemetry.                  |
| `defer candidates`                | A module of 3 KB or more runs less than 2% of its bytes at paint.                                        | Move its import behind `import()`.                                                                     |
| `cold bytes at paint`             | The browser fetches and parses a module, but the first paint reads little of it.                         | Defer the import. If one startup call pulls in a large SDK, defer the call.                            |
| `static pre-paint transfer`       | Chunks download before the first paint and run after it.                                                 | Load them on demand, or remove them from the initial tags and preloads.                                |
| `large modules executed at paint` | A large module runs at paint. This does not prove that the first render needs its contents.              | Check how much of it the first render reads. Split the rest behind `import()`.                         |
| `sibling variant groups`          | A family of locales or themes loads, but one session uses only one of them.                              | Keep the default in the entry. Load the active variant with `import()`.                                |
| `statically retained imports`     | One module keeps 100 KB or more on the initial load. This lead needs the build report.                   | Use [`what-if` and `cut`](#find-code-that-can-load-later) to find the import to edit.                  |

`node-scan` and `worker-scan` print other leads. [`AGENTS.md`](https://github.com/rolldown/rolldown/blob/main/packages/perf-kit/AGENTS.md) explains each lead and the order to fix them in.

## Run the commands yourself

You can also run the commands without an agent. The [README](https://github.com/rolldown/rolldown/blob/main/packages/perf-kit/README.md) lists every command, flag and output file.

### A page

Run each command as `npx @rolldown/perf-kit <command>`.

1. Build the app and run `scan --app <dir>`. The first scan pins the baseline and saves a copy of the build as the original. Later commands remember the target. To add client-side routes, use `--pages /pricing,/settings`.
2. Read the leads in the output. Each lead names a next step.
3. Make one change. Rebuild the app and run its tests.
4. Run `scan --no-early-stop`. If it reports an improvement beyond noise, run `scan --pin`. Otherwise, revert the change.
5. When you are done, run `scan --parity` and `verdict`.

### A Node.js or Deno server

```sh
$ npx @rolldown/perf-kit node-scan --entry dist/server.js --ready port:3000
```

`--ready` declares when the server is up: `port:<n>`, `stdout:<regex>`, an `http://` URL that answers 2xx, or `exit`. perf-kit pins it with the target.

`--exec deno` runs the server with Deno. Bun is not supported. After a kept change, run `node-scan --pin`.

### A Cloudflare Worker

Run these commands in the app folder:

```sh
$ npm i -D @rolldown/perf-kit
$ npx perf-kit worker-config --app .
$ npx vite build --config perf-kit.vite.config.ts
$ npx perf-kit worker-scan --app . --request /
```

`worker-config` writes a Vite config that wraps the config of the app. `--request` adds a route to call after startup. After a kept change, run `npx perf-kit worker-scan --pin`. The numbers come from the local CPU. Use `wrangler check startup`, before and after, for the number that Cloudflare reports.

### A deployed site

```sh
# Measure the deployed site in your own logged-in Chrome
$ npx @rolldown/perf-kit field --config field.json

# Save one deployed variant as a recording
$ npx @rolldown/perf-kit record --config field.json --variant base

# Serve the recording to a headless Chrome, with local builds in place of the deployed files
$ npx @rolldown/perf-kit replay --archive <dir> --dist base=<dir> --dist candidate=<dir>
```

Before you compare builds, check the recording with `--calibrate`. The [README](https://github.com/rolldown/rolldown/blob/main/packages/perf-kit/README.md) shows how. The output and the recordings hold real responses from your account, so perf-kit writes them outside the repository.

### Find code that can load later

Run each command as `npx @rolldown/perf-kit <command>`. These commands need the [build report](#turn-on-the-build-report), and they do not need a browser run. Their numbers are leads, not measurements, so check each change with `scan`.

| Command               | What it answers                                                                                                                |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `graph`               | Modules ranked by retained size: the bytes that leave the initial load when one import becomes dynamic, with the chain to cut. |
| `what-if <module>...` | The exact modules and bytes that a deferral frees. With 2 or more modules, it prices them as one plan.                         |
| `cut <module>`        | The fewest import statements to make dynamic, when several paths reach the module.                                             |
| `why <module>`        | The direct static importers of the module, and one import chain from the entry.                                                |
| `graph-diff`          | The modules that entered or left the initial load since the pinned baseline, with their bytes.                                 |

`cut` helps when several paths reach a module. In this example, 3 modules import `d3.js`, so an edit of each direct import takes 3 edits. `cut` finds 2:

```dot
digraph {
    rankdir=TB;
    bgcolor="transparent";
    node [shape=box, style="filled,rounded", fontname="Arial", fontsize=12, margin="0.2,0.1", color="${#3c3c43|#dfdfd6}", fontcolor="${#3c3c43|#dfdfd6}"];
    edge [fontname="Arial", fontsize=10, color="${#3c3c43|#dfdfd6}", fontcolor="${#3c3c43|#dfdfd6}"];

    main [label="main.js\n11.7 KB", fillcolor="${#fff0e0|#4a2a0a}"];
    sparkline [label="sparkline.js\n4.9 KB", fillcolor="${#dbeafe|#1e3a5f}"];

    subgraph cluster_leaves {
        label="Leaves the initial load: 273.6 KB";
        labeljust="l";
        labelloc="b";
        fontname="Arial";
        fontsize=11;
        fontcolor="${#3c3c43|#dfdfd6}";
        style="dashed,rounded";
        color="${#d44803|#ff712a}";

        charts [label="charts.js\n19.6 KB", fillcolor="${#dcfce7|#14532d}"];
        bar [label="bar-chart.js\n29.3 KB", fillcolor="${#dcfce7|#14532d}"];
        line [label="line-chart.js\n29.3 KB", fillcolor="${#dcfce7|#14532d}"];
        d3 [label="d3.js\n195.3 KB", fillcolor="${#dcfce7|#14532d}"];
    }

    main -> charts [color="${#d44803|#ff712a}", penwidth=2, style=dashed, label="  edit 1", fontcolor="${#d44803|#ff712a}"];
    main -> sparkline;
    charts -> bar;
    charts -> line;
    bar -> d3;
    line -> d3;
    sparkline -> d3 [color="${#d44803|#ff712a}", penwidth=2, style=dashed, label="  edit 2", fontcolor="${#d44803|#ff712a}"];
}
```

```text
min cut for src/d3.js - 2 import edge(s) to make dynamic (naive what-if would touch 3):
  src/main.js  ->  src/charts.js
  src/sparkline.js  ->  src/d3.js
make each import dynamic (import()) -> frees 273.6KB / 4 module(s) from the initial load.
```

Edit 1 also moves `charts.js` and the 2 chart modules. Check that the first render does not need them.

## Limitations

- Lab numbers are valid only on one machine and with the pinned throttle. The signal is the difference between two builds.
- A deferral moves work and does not delete it. The first request or interaction that needs the module pays for its load.
- The rendered-output check sees the page as the server sends it, without interaction. It does not compare states behind a click, a login or live data, so keep the app's own tests green.
- perf-kit does not collect metrics from real users. Field mode measures one account, one network and one machine.
- `--ready exit` and a `stdout:` pattern that starts with `^` time out under the inspector at this time.
