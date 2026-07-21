import { describe, expect, test } from 'vitest';
import { editFile, page, untilBrowserLogAfter, waitForBuildStable } from '~utils';

// Port of the removed Rust fixture
// `crates/rolldown/tests/rolldown/topics/hmr/accept-outside-circular`: same
// modules, same edges (`b`/`c` form a circle, `main`'s self-accept is the only
// acceptance and sits outside it), same edit (`c`: 'c' → 'cc'). The fixture
// never executed (`expectExecuted: false`) and only asserted the server's
// full-reload decision; that decision now lives in the client, so it is only
// observable here in a real page. Browser adaptations: the `node:assert`
// checks become DOM renders asserted by this spec, and the fixture's accept
// callback (dead code — it dereferenced `newMod.a` on a module with no
// exports) becomes a bare self-accept, which registers the same boundary.
//
// Today the client walk treats any circular import chain as a reload reason
// (`bubble()` stops when it re-meets an ancestor). If the walk ever learns to
// cross circles to an outside boundary (Vite's `propagateUpdate` behavior),
// this becomes a hot update — flip the marker assertion to `'alive'`.

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = () =>
  page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

describe('hmr-accept-outside-circular', () => {
  test('renders the chain through the circle', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.chain')).toBe('c');
  });

  test('editing inside the circle reloads onto fresh content', async () => {
    await waitForBuildStable();
    await plantMarker();

    // The reload must be attributed to the circle, not to a missing boundary
    // (`main` self-accepts, so a plain no-boundary reason would be a bug).
    await untilBrowserLogAfter(
      () => editFile('c.js', (code) => code.replace("export const c = 'c'", "export const c = 'cc'")),
      /full reload: circular import chain between `[^`]*b\.js` and `[^`]*c\.js`/,
    );
    await expect.poll(() => page.textContent('.chain')).toBe('cc');

    // A reload, never a silently stale page: the marker is gone AND the fresh
    // value rendered.
    await expect.poll(readMarker).toBe(null);
    await waitForBuildStable();
  });
});
