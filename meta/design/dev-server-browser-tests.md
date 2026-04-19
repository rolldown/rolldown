# Dev Server Browser Tests

## Summary

The dev server browser tests verify HMR and lazy compilation using real Playwright browser instances against running dev servers. All browser tests live in a single file (`browser.spec.ts`) with shared setup — this is a deliberate choice to avoid Windows CI flakiness caused by Vitest's `forks` pool.

## Architecture

```
packages/test-dev-server/tests/
  browser.spec.ts              # All browser tests (HMR + lazy compilation)
  vitest-setup-browser.ts      # Shared setup: servers, browser, pages
  vitest.config.browser.mts    # Vitest config
  test-utils.ts                # Helpers (editFile, waitForBuildStable, etc.)
  playground/
    hmr-full-bundle-mode/      # HMR test fixture (copied to tmp at runtime)
    lazy-compilation/           # Lazy compilation test fixture
```

The setup file (`vitest-setup-browser.ts`) runs as Vitest `setupFiles` (same process as tests):

1. Kill any leftover processes on ports 3636/3637
2. Recreate `tmp-playground/` from `playground/` (always fresh copy)
3. Start two dev servers (`pnpm serve`) — HMR on 3636, lazy on 3637
4. Launch Chromium via Playwright, create pages
5. Navigate only the HMR page — the lazy page is **not** navigated in setup to avoid warming the lazy-compilation server (see below)
6. Expose pages as `global.__page` and `global.__lazyPage`

### Cold Lazy Compilation

The lazy-compilation test navigates its page inside the test itself, not in `beforeAll`. This is intentional: `main.js` triggers `import('./lazy-module.js')` after 1 second. If the page were navigated during setup, the dynamic import would fire during the HMR tests, warming the server's lazy-compilation state before the lazy test runs. Navigating in the test ensures we exercise the cold `/@vite/lazy` compilation path.

Teardown closes the browser and kills dev servers via `killPort()`.

## Why a Single Test File

Vitest's `forks` pool creates a separate worker process per test file. On Windows, splitting tests across multiple files causes flaky failures:

1. Worker A (file 1) starts dev servers as child processes
2. Worker A finishes, exits — but its dev servers become orphaned on Windows
3. Worker B (file 2) starts, calls `killPort()` to clean up orphaned servers
4. `killPort` (using `taskkill`) doesn't always finish cleanup before Worker B needs the ports
5. Worker B crashes with "Worker exited unexpectedly"

With a single file: one fork, one setup, one teardown, no cross-process coordination.

## Port Configuration

Ports are defined in two places that must stay in sync:

| Server | `src/config.ts`                         | `playground/*/dev.config.mjs`         |
| ------ | --------------------------------------- | ------------------------------------- |
| HMR    | `CONFIG.ports.hmrFullBundleMode = 3636` | `hmr-full-bundle-mode/dev.config.mjs` |
| Lazy   | `CONFIG.ports.lazyCompilation = 3637`   | `lazy-compilation/dev.config.mjs`     |

Note: `waitForBuildStable(port)` takes port as its **first** argument. Passing the wrong port causes silent 30-second timeouts (it polls a non-existent server until the timeout fires).

## Adding New Browser Tests

1. Create a playground under `playground/` with `dev.config.mjs`, `package.json`, and source files
2. Add the port to `src/config.ts` and `dev.config.mjs`
3. Add server startup/teardown to `vitest-setup-browser.ts`
4. Add tests to `browser.spec.ts` in a new `describe` block
5. Keep everything in the single test file (see above for why)

## Related

- [lazy-compilation](./lazy-compilation.md)
- [watch-mode](./watch-mode.md)
- PR: https://github.com/rolldown/rolldown/pull/7974
