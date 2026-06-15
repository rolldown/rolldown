# Coding agent guides for `packages/test-dev-server/tests`

## Testing dev-server behavior

Most dev-engine regressions (HMR, lazy compilation, error recovery) are tested as **browser playgrounds**, not unit tests. A playground is a tiny app the in-process dev server serves to a real Chromium page.

Use:

```text
playground/<name>/
```

Each playground normally contains:

```text
dev.config.mjs            # rolldown dev config (browser platform, no dev.port)
index.html                # served at /
main.js                   # entry; relative input paths resolve from this dir
package.json              # workspace member (copy an existing one)
__tests__/<name>.spec.ts  # the spec (stays in source, never copied)
```

**Multiple spec files in one playground run concurrently** (file parallelism), each forking its own dev server against the shared `playground-temp/<name>/` copy. That is safe only when the scenarios are **disjoint** — each spec navigates its own DOM and edits only its own files (how `lazy-compilation`'s four specs coexist). When scenarios share one bundle/entry, so an edit by one would rebuild another spec's page, put them in a **single spec file** instead (why `hmr-full-bundle-mode`'s scenarios are one spec).

The spec imports helpers from the `~utils` alias; the harness has already started the server and navigated `page`:

```ts
import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

describe('<name>', () => {
  test('applies an HMR update', async () => {
    editFile('main.js', (code) => code.replace('hello', 'world'));
    await expect.poll(() => page.textContent('h1')).toBe('world');
  });
});
```

Synchronize on the server's async work — never `sleep`. Poll the DOM with `expect.poll`, `await waitForBuildStable()` before a follow-up edit, or wait on a browser log with `untilBrowserLogAfter`.

Build once (after changing rust or the dev-server `src/`):

```sh
just build-rolldown
pnpm --filter @rolldown/test-dev-server build
```

Run a focused playground with (from `packages/test-dev-server/tests/`):

```sh
pnpm exec vitest run --config=vitest.config.e2e.mts playground/<name>
```

Run the whole browser suite with:

```sh
pnpm test:browser
```

## Cold-start playgrounds

Some lazy-compilation bugs only reproduce on the **first** request to a fresh server. Add `__tests__/serve.ts` so the harness starts the server but does not navigate, and the spec fires the first request itself:

```ts
import type { DevServerHandle, ServeContext } from '~utils';

export async function serve(ctx: ServeContext): Promise<DevServerHandle> {
  return ctx.createServer();
}
```

## When to use the node fixtures suite

Use `fixtures/<name>/` + `fixtures.test.ts` (run with `pnpm test:fixtures`) for the **node** platform — the dev server building to disk and running the built artifact as a `node` child process.

Do not add ordinary HMR / lazy / overlay regressions there when a browser playground can cover the behavior.
