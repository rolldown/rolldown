# Dev Server Test Harness

## Summary

The `@rolldown/test-dev-server` browser suite drives a real Chromium page against
the rolldown dev engine (HMR, lazy compilation, error overlay). The server is
**Vite's full bundle mode** (`experimental.bundledDev`), loaded at runtime from
the Vite checkout at `vite/` (repo root, a gitignored clone of vitejs/vite
`rolldown-canary` rebased onto `main`) with its `rolldown`
resolution linked to the workspace's `packages/rolldown` — the harness adds only
test instrumentation on top (see [The Vite backend](#the-vite-backend)). It runs
**in-process**: each spec file starts the dev server inside its own vitest worker
on an OS-assigned port, connects to one shared Chromium, and tears it down via the
server's own `close()`. Playground discovery is derived from each spec's own path,
so there is no central registry — **adding a test is a folder plus a spec, with
zero central edits**. The companion node **fixtures** suite (a custom dev server
building to disk, the artifact run as a child process — Vite's bundled dev is
client-environment-only, so that platform cannot run on it) shares the status
helpers but is otherwise separate.

## Principles

### In-process, auto-port, spec-path discovery

The dev server runs in-process in the test worker, binds port 0, and the test
reads the resolved URL back — no port is ever hand-picked. Discovery is the spec
file's own path: `vitest.config.e2e.mts` globs `playground/**/*.spec.[tj]s`, the
playground name is regexed out of each spec path, and global setup copies only
the selected playgrounds into `playground-temp/`. **Adding a test is a folder + a
spec — zero central edits.**

### Listen-before-build

The HMR runtime needs the websocket port known up front. On the **node**
platform the server binds the socket **first**, reads the bound port, injects it
into `experimental.devMode.port`, then builds. On the **browser** platform Vite
manages the client's websocket address itself; the harness only reserves an
OS-assigned port before `createServer` because Vite treats `port: 0` as "use
the default port" rather than "let the OS pick" (see `getFreePort` in
`src/vite-server.ts`).

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
  reached by URL on the same server. Vite's html pipeline can serve multiple
  HTML entries from one root, but every existing playground uses co-tenant, so
  prefer it for consistency.

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
(`untilBrowserLogAfter`) for things with no DOM signal (reconnect, full
reload). Only string-only console arguments are matchable (object args render
as a preview, not JSON).

Browser specs assert on **Vite's own signals**:

| Signal        | Where                        | Marker                                              |
| ------------- | ---------------------------- | --------------------------------------------------- |
| WS connected  | browser log                  | `[vite] connected.`                                 |
| Patch applied | browser log                  | `[vite] hot updated: …`                             |
| Build error   | server log                   | `✘ Build error: …`                                  |
| HMR / reload  | server log                   | `hmr update …`, `hmr invalidate …`, `page reload`   |
| Error overlay | DOM (`<vite-error-overlay>`) | `errorOverlay()` / `errorOverlayText()` in `~utils` |

The overlay renders in a **shadow root**, so `locator(...).textContent()` on
the host element returns nothing — always go through the `~utils` helpers. The
node fixtures assert on the rolldown runtime's `[hmr]: …` markers instead.

## Implementation (as built)

### The Vite backend

The browser platform is Vite's full bundle mode; the harness owns none of the
serving. What the harness does own lives in `src/vite-server.ts`:

- **Config translation.** `dev.config.mjs` → Vite inline config:
  `experimental.bundledDev: true`, the playground copy as `root` (its
  `index.html` module script is the entry), fixture plugins passed through
  (Vite 8 runs rolldown natively), `assetsInlineLimit: 0` for asset-request
  assertions, `treeshake` forwarded. Full bundle mode forces
  `devMode.lazy: true`, so lazy compilation is always on in browser runs.
- **`vite` is not a package dependency.** It is dynamic-imported by file URL
  from the checkout's built dist (`loadVite()`), with local structural types
  for the API slice the harness touches. Node-platform fixtures and CI jobs
  that never run browser tests work without the checkout; a missing dist
  fails with a "run `just setup-vite`" hint.
- **Test instrumentation** (`createHarnessPlugin`): the `/_dev/status`
  middleware; `buildSeq` counts `buildStart` plus broadcast
  `update`/`full-reload` payloads and deliberately **not** `error` payloads
  (the server replays the cached error to every fresh client, and the
  conservative-rebuild specs assert a refresh on a broken build does not move
  `buildSeq`); `moduleRegistrationSeq` counts `vite:module-loaded` events;
  bundle state is read live from `bundledDev.devEngine.getBundleState()`.
- **Workarounds for upstream gaps**, each commented `WORKAROUND` in the code
  and deletable once fixed in Vite:
  - _Recovery reload_: upstream only full-reloads after a successful build
    when a reload was already pending from HMR, so clients on the error
    overlay or the fallback page never learn an errored → ok build succeeded.
    The plugin observes broadcast `error` payloads and, on the next successful
    `generateBundle`, clears the cached error and reloads after
    `ensureLatestBuildOutput()`.
  - _Stale-error replay guard_: upstream clears `lastBuildError` only in
    `onOutput`, and the client hard-reloads when its first update meets an
    existing overlay — the reconnect would get a stale error replayed. A
    `vite:client:connect` listener (registered before Vite's own replay
    listener) drops the stale error when the tracked build state is healthy.

**The checkout stays unpatched.** No Vite source edits on the rolldown side.
Fixes and test adjustments belong on the vitejs/vite `rolldown-canary` branch,
which both this harness and `packages/vite-tests` track. Everything
environment-specific happens in untracked files, via the
`scripts/src/setup-vite/` script (`just setup-vite`, idempotent, `vp`-only;
its checkout step is also reused by `packages/vite-tests`):

1. ensure `vite/` is at the latest `rolldown-canary` rebased onto `main`
   (clone if missing, update otherwise); a checkout taken over by the
   developer (dirty, or off `rolldown-canary`) is built exactly as-is,
2. `vp install --frozen-lockfile` (vp delegates to the checkout's pinned
   pnpm; this also resets a previous step-4 swap, so the build always uses
   Vite's own pinned rolldown),
3. build `packages/vite` via its own `build` script (`vp run build`),
4. swap `vite/packages/vite/node_modules/rolldown` to a symlink at the
   workspace's `packages/rolldown`, so Vite's dist resolves the local binding
   at runtime. Any install inside the checkout resets this, so re-run the
   script after such an install.

Repo-wide tools ignore `vite/**` (a `.gitignore`
entry covers gitignore-respecting walkers like oxfmt, plus `.typos.toml` and
`.ls-lint.json` entries) — a repo-wide `vp fmt --write` must never touch
files inside `vite/`. On CI, the checkout is created on demand: the dev-server
workflow via the setup step, the vite-tests jobs via `run.ts` (which clones
the checkout locally to run Vite's own suite); every other job needs no Vite
checkout.

### Server entry point (`src/`)

- `createDevServer(config, opts?) → { url, port, close }` (`src/dev-server.ts`,
  exported from `src/index.ts` alongside `loadDevConfig(dir)` and a `Logger`
  type) dispatches on `build.platform`: `browser` → the Vite backend above,
  anything else → the node transport (`DevServer` +
  `FullBundleDevEnvironment`). On both paths a resolved promise means the
  initial build (or its error) has settled: Vite's `listen()` fires the build
  without awaiting it, so the browser path polls the `initialBuildCompleted`
  flag `BundledDev` sets once its own `waitForInitialBuildFinish()` (which
  polls `memoryFiles`) settles and the one-shot ready reload has been
  broadcast — `ensureCurrentBuildFinish()` alone can resolve before the build
  starts or before `onOutput` has stored the files; the node path awaits a
  first-output latch (`waitForFirstOutput`) for the same lag.
- **`close()`.** Browser: Vite's `server.close()` cascades into
  `bundledDev.close()` → `devEngine.close()`. Node: stop the ws server,
  terminate clients, `closeAllConnections()`, `httpServer.close()`,
  `env.close()`. Both release the watcher/tokio threads so a vitest fork
  exits, and a second engine can start in the same process after the first
  closes — covered by `dev-engine-close.test.ts` + `dev-engine-close-child.mjs`.
- `serve()` (the CLI/fixtures path) loads the cwd config and dispatches the
  same way; the stdin `'r'` rebuild trigger is wired on the node path only.
- **Injectable `Logger`.** The node transport logs through it directly; the
  browser path adapts it to Vite's `customLogger` (`toViteLogger`). The
  harness passes an in-memory logger so server-side output lands in
  `serverLogs`.
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
button, getting a virgin first-fetch for that scenario — as cold as a dedicated
server would give. The single config is the union of each scenario's needs:
currently just `viteAliasPlugin` (aliased-import; inert elsewhere).

### Knip / workspace

Playgrounds are pnpm workspace members via the
`packages/test-dev-server/tests/playground/*` glob in `pnpm-workspace.yaml`. The
consolidated `lazy-compilation` playground is one such member; its single
`knip.jsonc` entry globs the nested scenario sources (`*/*.js`) plus the specs
and `serve.ts`. `test-utils.ts` + `dev-engine-close-child.mjs` (referenced via
the `~utils` alias and an execa path string, which knip can't trace) are entries
in the `tests` workspace.

## Open follow-ups

- **Upstream the two Vite bundled-dev fixes.** The recovery reload and the
  stale-error replay (see [The Vite backend](#the-vite-backend)) are genuine
  upstream gaps; once the fixes land on vitejs/vite `rolldown-canary`, delete
  the `WORKAROUND` blocks in `src/vite-server.ts`.
- **Client-reconnect gate after reloads.** Add
  `untilBrowserLogAfter(() => page.reload(), [/\[vite\] connected\./])` so an
  edit fired after a reload can't be lost to a not-yet-reattached websocket —
  the marker already exists, no runtime change needed.

## Related

- [dev-engine](../dev-engine/implementation.md) — the engine the harness
  exercises ([design.md](../dev-engine/design.md) for its principles).
- [lazy-compilation](../lazy-compilation/implementation.md), [watch-mode](../watch-mode/implementation.md).
