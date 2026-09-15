import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

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
// Reaching the `b`/`c` back edge used to end the walk in a full reload. Vite's
// bundled-dev client now skips a parent it is already inside of and keeps
// going (vitejs/vite#23259), so the walk reaches `main`'s self-accept outside
// the circle and this is a hot update. The marker assertion tells them apart.

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

  test('editing inside the circle hot-updates via the boundary outside it', async () => {
    await waitForBuildStable();
    await plantMarker();

    editFile('c.js', (code) => code.replace("export const c = 'c'", "export const c = 'cc'"));
    await expect.poll(() => page.textContent('.chain')).toBe('cc');

    // A hot update, never a reload onto the same content: `cc` rendered AND the
    // marker survived.
    expect(await readMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
