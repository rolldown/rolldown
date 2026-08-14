import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// Port of the removed Rust fixture
// `crates/rolldown/tests/rolldown/topics/hmr/no-accept-outside-circular`: the
// same graph as `hmr-accept-outside-circular` but with NO acceptance anywhere,
// and the same edit (`c`: 'c' → 'cc'). The walk from the edit has to pass
// through the `b`/`c` circle before it can conclude there is no boundary; the
// client must end in a clean full reload onto fresh content, not an infinite
// walk or a stale page. The fixture never executed (`expectExecuted: false`)
// and only asserted the server's full-reload decision, which now lives in the
// client — observable only here. Browser adaptation: the `node:assert` check
// becomes a DOM render asserted by this spec.

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = () =>
  page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

describe('hmr-no-accept-outside-circular', () => {
  test('renders the chain through the circle', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.chain')).toBe('c');
  });

  test('editing inside the circle with no boundary anywhere reloads onto fresh content', async () => {
    await waitForBuildStable();
    await plantMarker();

    editFile('c.js', (code) => code.replace("export const c = 'c'", "export const c = 'cc'"));
    await expect.poll(() => page.textContent('.chain')).toBe('cc');

    // A reload, never a silently stale page: the marker is gone AND the fresh
    // value rendered.
    await expect.poll(readMarker).toBe(null);
    await waitForBuildStable();
  });
});
