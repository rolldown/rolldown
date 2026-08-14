import { describe, expect, test } from 'vitest';
import {
  editFile,
  page,
  plantReloadMarker,
  readReloadMarker,
  waitForBuildStable,
} from '~utils';

// Same as `hmr-dynamic-import-accept-dep`, but with `devMode.lazy: true`. Under lazy
// compilation `foo` compiles on demand behind a `?rolldown-lazy=1` proxy, so the importer
// walk sees the proxy chain instead of the real `app -> foo` dynamic edge and can't reach
// `app`'s accept-dep boundary — the edit full-reloads.

describe('hmr-lazy-dynamic-import-accept-dep', () => {
  // Skipped together with the gap test below: with every test in the file skipped,
  // vitest never runs the per-file browser + dev-server boot. The initial lazy render
  // is already covered by `lazy-compilation/__tests__/basic.spec.ts`.
  test.skip('renders the dynamically-imported value', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.foo')).toBe('foo-v1');
  });

  // KNOWN LAZY GAP: lazy dynamic-import HMR bubbling is not implemented — a lazy dynamic
  // edge full-reloads today (`crates/rolldown_common/src/ecmascript/ecma_view.rs` excludes
  // `?rolldown-lazy=1` importers from the walkable set). Verified: this currently wipes the
  // marker (full reload). Unskip once lazy dynamic-import HMR lands.
  test.skip('editing the dynamically-imported dep hot-updates via the importer accept-dep', async () => {
    await waitForBuildStable();
    await plantReloadMarker();

    editFile('foo.js', (code) => code.replace("'foo-v1'", "'foo-v2'"));
    await expect.poll(() => page.textContent('.foo')).toBe('foo-v2');

    // No full reload happened: the boundary walk crossed the lazy dynamic edge to `app`.
    expect(await readReloadMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
