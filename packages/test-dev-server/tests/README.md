# test-dev-server tests

End-to-end tests for `@rolldown/test-dev-server` — the small Vite-style dev
server that exercises rolldown's **dev engine** (HMR, lazy compilation, error
recovery) inside this repo. There are two suites:

| Suite               | Config                       | What it drives                                                                      |
| ------------------- | ---------------------------- | ----------------------------------------------------------------------------------- |
| **browser** (`e2e`) | `vitest.config.e2e.mts`      | A real Chromium page against an **in-process** dev server. HMR + lazy + overlay.    |
| **fixtures** (node) | `vitest.config.fixtures.mts` | The dev server building to **disk**, with the built artifact run as a `node` child. |

This README is about the **browser** suite — that is where you add most tests.
The architecture (and the "why") is documented in
[`meta/design/dev-server-test-harness.md`](../../../meta/design/dev-server-test-harness.md);
read its **Principles** and **Implementation** sections if you touch the harness
itself.

## TL;DR — add a browser test

A playground is a tiny app the dev server serves. Adding a test is **a folder
and a spec** — no port to pick, no central registry to edit.

1. Create `playground/<name>/` with:
   - `dev.config.mjs` — the rolldown dev config (see [Anatomy](#anatomy-of-a-playground)). **No `dev.port`.**
   - `index.html` + your source modules (`main.js`, etc.).
   - `package.json` — copy an existing one; lets you `pnpm dev` it standalone and declares deps.
2. Create `playground/<name>/__tests__/<name>.spec.ts`:

```ts
import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

describe('<name>', () => {
  test('does the thing', async () => {
    // The harness already started the server and navigated `page` to it.
    expect(await page.textContent('h1')).toBe('Hello');

    editFile('main.js', (code) => code.replace('Hello', 'World'));
    await expect.poll(() => page.textContent('h1')).toBe('World');
  });
});
```

That's it. The harness discovers the playground from the spec's path, copies it
to `playground-temp/<name>/`, starts an in-process dev server on an OS-assigned
port, opens a page, and navigates to it. Cleanup closes everything.

## Running

Prerequisites (once / after changing rust or the dev-server `src/`):

```bash
just build-rolldown                              # builds rolldown + the native binding
pnpm --filter @rolldown/test-dev-server build    # builds the dev server to dist/ (tests import from it)
pnpm --filter @rolldown/test-dev-server-tests exec playwright install chromium
```

Then, from `packages/test-dev-server/tests/`:

```bash
pnpm test:browser        # the browser e2e suite (all playgrounds)
pnpm test:fixtures       # the node fixtures suite
pnpm test                # both
pnpm typecheck           # tsc --noEmit (authoritative; see IDE note below)

# a single playground:
pnpm exec vitest run --config=vitest.config.e2e.mts playground/<name>

# watch the browser (headed) while debugging:
DEBUG_BROWSER=1 pnpm test:browser
```

If you change anything under the dev server's `src/`, **rebuild it** (step 2
above) — the tests import the compiled `dist/`, not the TypeScript source.

## Anatomy of a playground

```
playground/<name>/
  dev.config.mjs            # rolldown dev config (browser platform, no dev.port)
  index.html               # served at `/`; references `/main.js`
  main.js  …               # your source; relative `input` paths resolve from this dir
  package.json             # standalone `pnpm dev` + deps (the harness does NOT launch through it)
  __tests__/
    <name>.spec.ts         # the test (stays in source — never copied to playground-temp)
    serve.ts               # OPTIONAL — see "Cold-start playgrounds"
```

A minimal `dev.config.mjs`:

```js
import { defineDevConfig } from '@rolldown/test-dev-server';

export default defineDevConfig({
  platform: 'browser',
  build: {
    input: { main: 'main.js' },
    platform: 'browser', // browser => served from memory + HMR; node => served from disk
    treeshake: false,
    experimental: { devMode: {} }, // { lazy: true } for lazy compilation
  },
});
```

Test-only rollup plugins (transform delays, fake slow builds, etc.) go right in
`dev.config.mjs` `build.plugins` — they travel with the config in-process. See
`hmr-full-bundle-mode/dev.config.mjs` for examples.

**The playground is copied to `playground-temp/<name>/` per run** and your test
edits that copy, so the source under `playground/` stays pristine. `__tests__/`,
`dist/`, and `node_modules/` are excluded from the copy.

## The `~utils` toolkit

Everything is imported from the `~utils` alias (re-exported from
`playground/test-utils.ts`). The current spec's server and page are live
bindings — no setup needed.

| Binding                                        | What it is                                                                      |
| ---------------------------------------------- | ------------------------------------------------------------------------------- |
| `page`                                         | the Playwright `Page`, already navigated (except cold-start playgrounds)        |
| `serverUrl`                                    | the running server's URL (OS-assigned port). `page.goto(serverUrl)` to navigate |
| `testDir`                                      | absolute path to this spec's `playground-temp/<name>/` copy                     |
| `browserLogs` / `browserErrors`                | arrays of captured `console` text / page errors (`console.debug` included)      |
| `serverLogs`                                   | array of captured server-side log output                                        |
| `editFile(name, code => newCode)`              | edit a file in `testDir` (triggers the watcher). Warns on a no-op edit          |
| `readFile` / `addFile` / `removeFile`          | other `testDir`-relative file ops                                               |
| `waitForBuildStable()`                         | resolve once `buildSeq` stops changing (the debounce window closed)             |
| `waitForNextBuild(seq)`                        | resolve once a build past `seq` completed                                       |
| `getBuildSeq()` / `getModuleRegistrationSeq()` | read the `/_dev/status` counters                                                |
| `untilBrowserLogAfter(op, target)`             | run `op`, resolve when the browser logs `target` (string/regex/ordered list)    |

## Writing reliable tests

The dev server is asynchronous: a file edit kicks off a debounced rebuild, the
patch is pushed over a websocket, the browser applies it. **Never `sleep`** —
synchronize on one of these instead:

- **Assert on the DOM with polling.** `expect.poll` retries until the HMR patch
  lands:
  ```ts
  editFile('hmr.js', (c) => c.replace('hello', 'world'));
  await expect.poll(() => page.textContent('.hmr')).toBe('world');
  ```
- **Wait for the build to settle before the _next_ edit.** The watcher debounces;
  editing again before the window closes can coalesce or miss the change:
  ```ts
  await expect.poll(() => page.textContent('.hmr')).toBe('world');
  await waitForBuildStable(); // <- debounce window closed
  editFile('hmr.js', (c) => c.replace('world', 'again'));
  ```
- **Wait on browser logs** for things that have no DOM signal (a reconnect, a
  full reload, a specific HMR event):
  ```ts
  await untilBrowserLogAfter(() => page.reload(), [/Connection established/]);
  ```
  The runtime logs markers via `console.debug` (which Playwright captures);
  `[test-dev-server] hot updated: …`, `error overlay shown`, and `build ok`
  markers are emitted for assertions too.

Conventions that keep a shared page safe (tests in a file share one `page` and
run sequentially):

- **Edits are forward-only**, or restore what they changed — a later test must
  not depend on an earlier one's edits being reverted unless it reverts them.
- **Re-acquire element handles** after any reload (`page.locator(...)` / fresh
  `page.$`); a reload invalidates old handles.
- **Own disjoint files + DOM nodes** per scenario so one test can't perturb
  another's assertions.

## Cold-start playgrounds (`serve.ts`)

Some lazy-compilation regressions only reproduce on the **first** request to a
fresh server (the first request permanently changes later builds). For those,
add `__tests__/serve.ts` so the harness starts the server but does **not**
navigate — your spec fires the first request itself:

```ts
// __tests__/serve.ts
import type { DevServerHandle, ServeContext } from '~utils';

export async function serve(ctx: ServeContext): Promise<DevServerHandle> {
  return ctx.createServer(); // create the server, but DON'T navigate
}
```

```ts
// the spec then controls the cold first hit:
import { page, serverUrl } from '~utils';
await page.goto(serverUrl, { waitUntil: 'domcontentloaded' });
```

Put `{ retry: 0 }` on first-interaction-only regression tests so a retry can't
mask the bug by landing on the already-warmed path.

## Multiple scenarios in one playground

A playground can host several scenarios on one page + one config — Vite's `hmr`
co-tenant pattern. `lazy-compilation/` does this: `main.js` statically imports a
`setup.js` from each scenario sub-folder (`basic/`, `aliased-import/`,
`shared-module/`, `nested-dynamic-import/`), each scenario owns disjoint DOM
nodes (`#<scenario>-btn` / `-status` / `-log`), and there is one spec file per
scenario in `__tests__/` sharing one `serve.ts`. Each spec still gets its own
per-file server, so it can edit its scenario's files (`editFile('shared-module/
selectors.js', …)`) and reload without disturbing the others.

For the lazy scenarios this is safe even though each needs a cold server: a lazy
chunk is compiled only when its dynamic import fires, so co-tenant scenarios
never warm each other — a spec that clicks only its button gets a virgin
first-fetch for that scenario. Reach for a separate top-level playground only
when the server **config** genuinely can't be shared.

## The node fixtures suite (brief)

`fixtures.test.ts` + `fixtures/<name>/` test the **node** platform: the dev
server writes to disk, the test execs the built `dist/main.js` as a child
process, drives step-based edits (`*.hmr-N.*` replacement files with
`@restart` / `@reload` markers), and syncs via the `/_dev/status` helpers in
`src/dev-status.ts`. This suite still uses fixed ports + subprocesses by
necessity (the running artifact is the test subject). It is out of scope for the
browser-harness conventions above.

## Gotchas

- **Rebuild the dev server** (`pnpm --filter @rolldown/test-dev-server build`)
  after editing its `src/` — tests import `dist/`, not the source.
- **Do not edit the shared HMR runtime**
  (`crates/rolldown_plugin_hmr/src/runtime/runtime-extra-dev-default.js`).
  Dev-server-specific client behavior lives in `src/error-overlay.ts`.
- **A SIGKILL'd run can corrupt a source fixture.** `editFile` targets the temp
  copy, but if `git status` shows a `playground/` file changed, `git checkout`
  it before debugging further.
- **IDE note:** specs live in workspace-member dirs, so a `playground/tsconfig.json`
  exists purely so editors resolve `~utils`. If your editor still shows a false
  "Cannot find module '~utils'", reload the TS server — `pnpm typecheck` is the
  source of truth and will tell you the real story.
- `fileParallelism` is off and `retry` is on, intentionally (pre-Phase-4 — see
  the design doc). New flakes are real bugs or missing waits; add the right
  wait rather than leaning on retry.
