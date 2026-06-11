# Dev Server Test Harness

## Summary

The `@rolldown/test-dev-server` browser suite drives a real Chromium page against
the rolldown dev engine (HMR, lazy compilation, error overlay). It runs
**in-process**: each spec file starts the dev server inside its own vitest worker
on an OS-assigned port, connects to one shared Chromium, and tears it down via the
server's own `close()`. Playground discovery is derived from each spec's own path,
so there is no central registry — **adding a test is a folder plus a spec, with
zero central edits**. The companion node **fixtures** suite (the dev server
building to disk, the artifact run as a child process) is separate and out of
scope here. Current CI posture: `fileParallelism: false` with `retry` on — both
intentional (see [Open follow-ups](#open-follow-ups)).

## Principles

### In-process, auto-port, spec-path discovery

The dev server runs in-process in the test worker, binds port 0, and the test
reads the resolved URL back — no port is ever hand-picked. Discovery is the spec
file's own path: `vitest.config.e2e.mts` globs `playground/**/*.spec.[tj]s`, the
playground name is regexed out of each spec path, and global setup copies only
the selected playgrounds into `playground-temp/`. **Adding a test is a folder + a
spec — zero central edits.**

### Listen-before-build

The HMR runtime needs the websocket port baked into the bundle at build time, so
the server binds the socket **first**, reads the bound port, injects it into
`experimental.devMode.port`, then builds. That is what makes auto-port work
without touching the shared runtime (a relative-ws client would be the "right"
fix but is out of scope).

### One playground per server-config

A new top-level playground exists **only when the server config must differ**
(plugins, platform, lazy mode, …) — never because of scenario count. Scenarios
that can share a config share one playground. Within a playground there are two
ways to host several scenarios:

- **Co-tenant** (one page): the root `index.html` + entry statically import each
  scenario's module; each scenario owns **disjoint DOM nodes + files** so one
  cannot perturb another's assertions. (Vite's `hmr`; rolldown's
  `lazy-compilation`.)
- **Sub-page** (one page per scenario): a sub-folder with its own `index.html`,
  reached by URL on the same server. This needs the dev server to serve multiple
  HTML entries from one root — which test-dev-server does **not** do today (it
  emits a single `index.html` from the cwd), so rolldown uses co-tenant only.

**Lazy cold-start is compatible with co-tenancy.** A lazy chunk is compiled only
when its own dynamic import fires, so bundling several lazy scenarios into one
project never warms another's chunks. Each spec boots its own per-file (virgin)
server and triggers only its scenario, getting a first-fetch as cold as a
dedicated server would give. Disjoint lazy chunks + DOM nodes are sufficient
isolation.

### Serve-mode ladder

Most playgrounds take the default path; escalate only for a specific need:

| Need                                   | Mechanism                                                                                                    |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| Normal browser dev-server behavior     | Default: the harness starts the server and navigates in `beforeAll`                                          |
| Cold first request (no pre-navigation) | `__tests__/serve.ts` returns `ctx.createServer()` **without** navigating; the spec fires its own `page.goto` |
| A virgin server mid-file               | The spec creates/closes its own server per test (Vite's `client-reload` pattern)                             |

### Reload safety on a shared page

Tests in a file share one `page` and run sequentially; a reload affects
everything after it. Safety is by convention, not isolation:

- Scenarios own **disjoint files + DOM nodes**.
- **Edits are forward-only** (or revert what they change) — never assume pristine
  files after a reload.
- **Re-acquire element handles** after any reload; a reload invalidates them.
- Escalation ladder for destructive state: shared page → own sub-page → own page
  (`browser.newPage()`) → own server.

### Synchronize, never sleep

Two mechanisms. (1) `/_dev/status` polling — `waitForBuildStable`, `buildSeq`,
`moduleRegistrationSeq` — plus `expect.poll` on DOM text. (2) Browser-log gates
(`untilBrowserLogAfter`) for things with no DOM signal (reconnect, full reload).
The runtime's markers are emitted via `console.debug`, which Playwright captures:

| Event          | Marker                                        | Level |
| -------------- | --------------------------------------------- | ----- |
| Runtime loaded | `HMR runtime loaded <addr>`                   | debug |
| WS connected   | `[hmr]: Connection established with server`   | debug |
| Patch received | `[hmr]: Loading HMR patch: <path>`            | debug |
| Full reload    | `[hmr]: Full reload required, reloading page` | log   |

Only string-only markers are matchable (object args render as a preview, not
JSON). test-dev-server adds its own `[test-dev-server] hot updated: …`,
`error overlay shown: …`, and `build ok` markers from the injected overlay
client for post-apply / overlay assertions.

## Implementation (as built)

### Server entry point (`src/`)

- `createDevServer(config, opts?) → { url, port, close }` (`src/dev-server.ts`,
  exported from `src/index.ts` alongside `loadDevConfig(dir)` and a `Logger`
  type). Binds `opts.port ?? 0`, runs the initial build, and resolves once output
  is being served.
- **`close()`** composes: stop the ws server, terminate clients,
  `closeAllConnections()`, `httpServer.close()`, `env.close()`. `DevServer` is a
  config-taking class; `serve()` (the CLI/fixtures path) loads the cwd config and
  delegates to it, and is the only path that wires the stdin `'r'` rebuild
  trigger. `close()` releases the watcher/tokio threads so a vitest fork exits,
  and a second engine can start in the same process after the first closes —
  covered by `dev-engine-close.test.ts` + `dev-engine-close-child.mjs`.
- **`waitForFirstOutput`.** `env.run()` resolves when the engine settles, but the
  JS `onOutput` callback that fills `memoryFiles` can lag a tick; `createDevServer`
  awaits a first-output latch so a resolved start means the first bundle (or its
  error) is actually being served — a navigation never lands on the spinner.
- **Injectable `Logger`.** `DevServer` / `FullBundleDevEnvironment` / the
  dev-server plugin / the lazy middleware / `Clients` log through a `Logger`
  (default `console`); the harness passes an in-memory logger so server-side
  output lands in `serverLogs`.
- `DEV_SERVER_PORT` is **not** consulted by `createDevServer` (it binds
  `opts.port ?? 0`); it stays the fixtures/CLI channel consumed by `serve()`.

### Harness layout (`tests/`)

```
tests/
  vitest.config.e2e.mts            # discovery: include playground/**/*.spec.ts; ~utils alias; setup/globalSetup
  vitest.config.fixtures.mts       # node fixtures + dev-engine-close smoke test
  fixtures.test.ts                 # status helpers re-keyed to URL
  dev-engine-close.test.ts         # close path + restart-in-process smoke test
  src/
    dev-status.ts                  # URL-keyed /_dev/status helpers (shared by fixtures + browser)
    dev-engine-close-child.mjs     # bare-node child proving the engine releases the process
    utils.ts                       # fixtures dir helpers
  playground/
    vitest-global-setup.ts         # one chromium.launchServer(); selective copy → playground-temp/
    vitest-setup.ts                # per-file: derive testName/testDir, connect browser, start server or run serve.ts
    test-utils.ts                  # the ~utils surface (re-exports + editFile + untilBrowserLogAfter + status helpers)
    <name>/                        # a flat playground: one server config, one page
      __tests__/<name>.spec.ts     # the spec (lives in source, never copied)
      __tests__/serve.ts           # optional escape hatch (cold-start)
      dev.config.mjs               # no dev.port
      package.json  index.html  …  # the fixture (copied to playground-temp/<name>/)
    lazy-compilation/              # a co-tenant playground: one config, many scenarios
      __tests__/
        serve.ts                   # ONE cold-start serve shared by every scenario spec
        basic.spec.ts  aliased-import.spec.ts  shared-module.spec.ts  nested-dynamic-import.spec.ts
      dev.config.mjs  index.html  main.js   # one union config; main.js imports each scenario
      <scenario>/setup.js  …                # one sub-folder per scenario (sources + lazy modules)
      package.json
```

Notable points:

- **File names are kebab-case** (`vitest-setup.ts`, `vitest-global-setup.ts`) —
  the repo's `ls-lint` enforces it. `__tests__/` is kept (it is the temp-copy
  filter boundary that keeps specs + `serve.ts` out of the served fixture) via an
  `.ls-lint.json` ignore.
- **Status helpers live in `tests/src/dev-status.ts`** (URL-keyed, hook-free) so
  the node `fixtures.test.ts` imports them too; `test-utils.ts` re-exports thin
  wrappers that default the URL to the current spec's `serverUrl`.
- **`testDir` is the temp copy; `testPath` is the source spec.** `serve.ts` is
  resolved next to the source spec (`dirname(testPath)`), since `__tests__/` is
  excluded from the copy.
- **`build.cwd = testDir` injection.** In-process the worker's cwd is the tests
  dir, so the harness pins `cwd` to the playground copy when loading the config —
  otherwise relative `input` paths and the plugin's `index.html` lookup resolve
  against the wrong directory.
- The global-setup copy **excludes `node_modules`**: bare imports from
  `playground-temp/<name>/` resolve by walk-up to `tests/node_modules`
  (depth-insensitive), so copying pnpm's symlink forest is unnecessary.

### The `serve.ts` contract

A playground's optional `__tests__/serve.ts` exports
`serve(ctx) → Promise<DevServerHandle>`. `ctx` carries `{ testName, testDir,
page, createServer }`, where `createServer()` loads the playground config and
starts the in-process server (logger + cwd wired). The `lazy-compilation`
playground's four scenario specs share one `serve.ts` with the body
`return ctx.createServer()` — it creates the server but skips navigation, so each
spec fires the cold first `page.goto(serverUrl)` itself. The default path (HMR)
has no `serve.ts`: the harness starts the server and navigates.

### Lazy compilation: one project, many scenarios

The lazy-compilation regressions were originally four sibling playgrounds, each
with its own server config. They are now one playground with **one**
`dev.config.mjs` and one page: `main.js` statically imports a `setup.js` from
each scenario sub-folder (`basic/`, `aliased-import/`, `shared-module/`,
`nested-dynamic-import/`), each scenario owns disjoint DOM nodes
(`#<scenario>-btn` / `-status` / `-log`), and there is one spec file per scenario
in `__tests__/`. This works because compilation is lazy (see the co-tenancy
principle above): each spec boots its own per-file server and clicks only its
button, getting a virgin first-fetch for that scenario — exactly what four
separate servers used to give. The single config is the union of each scenario's
needs: `viteAliasPlugin` (aliased-import; inert elsewhere), `strictExecutionOrder`,
and `incrementalBuild` (shared-module, nested).

### Knip / workspace

Playgrounds are pnpm workspace members via the
`packages/test-dev-server/tests/playground/*` glob in `pnpm-workspace.yaml`. The
consolidated `lazy-compilation` playground is one such member; its single
`knip.jsonc` entry globs the nested scenario sources (`*/*.js`) plus the specs
and `serve.ts`. `test-utils.ts` + `dev-engine-close-child.mjs` (referenced via
the `~utils` alias and an execa path string, which knip can't trace) are entries
in the `tests` workspace.

## Open follow-ups

- **Lift the single-spec-file / `fileParallelism: false` constraint.** In-process
  removed the orphaned child servers that caused Windows `forks` flakiness, so
  multiple spec files should be safe — but running many `FullBundleDevEnvironment`
  builds concurrently across workers is untested at scale. Trial parallelism on a
  Windows CI branch; keep only if green.
- **Retire the retry crutch.** Once parallelism is proven, drop `retry` and any
  retry-reset `beforeEach`; remaining flakes are then real bugs or missing waits.
- **Client-reconnect gate after reloads.** Add
  `untilBrowserLogAfter(() => page.reload(), [/Connection established/])` so an
  edit fired after a reload can't be lost to a not-yet-reattached websocket — the
  marker already exists, no runtime change needed.

## Related

- [dev-engine](./dev-engine.md) — the engine the harness exercises.
- [lazy-compilation](./lazy-compilation.md), [watch-mode](./watch-mode.md).
