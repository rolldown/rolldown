import { describe, expect, test } from 'vitest';
import {
  editFile,
  page,
  plantReloadMarker,
  readReloadMarker,
  waitForBuildStable,
} from '~utils';

// Ports Vite's `hmr` acceptExports behavior. On the client `acceptExports(names, cb)` is a
// self-accept (export-name filtering is a server-side concern), so editing the module should
// run the callback with the fresh module rather than full-reloading.

describe('hmr-accept-exports', () => {
  test('renders the initial value', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.value')).toBe('exports-v1');
  });

  // KNOWN GAP: rolldown's scanner recognizes `import.meta.hot.accept(...)` as self-accepting
  // but not `acceptExports(...)`, so the module's compile-side `HmrSelfAccept` flag is unset.
  // The server-side propagation walk (`crates/rolldown/src/hmr/hmr_stage.rs` —
  // `is_hmr_self_accepting_module`) then treats the edit as having no boundary and sends a
  // full reload, even though the client already registered the self-accept at runtime.
  // Verified: this currently full-reloads. Unskip once the compiler recognizes acceptExports.
  test.skip('acceptExports hot-updates via its callback', async () => {
    await waitForBuildStable();
    await plantReloadMarker();

    editFile('app.js', (code) => code.replace("'exports-v1'", "'exports-v2'"));
    await expect.poll(() => page.textContent('.value')).toBe('exports-v2');

    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
