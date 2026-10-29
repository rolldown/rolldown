import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// Ports the retired node fixture `client-module-execution-status`: `common-child` has
// two dynamic-import parents in the module graph, but only `parent-executed` ran in
// this tab. Editing the child must hot-update through the executed parent's accept and
// leave the never-executed `parent-cold` inert — execution state is per-tab, decided by
// the client's own module cache.

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = () =>
  page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

const readExecuted = () =>
  page.evaluate(() => (window as unknown as { __executed?: string[] }).__executed ?? []);

describe('hmr-executed-importer-boundary', () => {
  test('renders the child value through the executed parent only', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.child')).toBe('child-v1');
    expect(await readExecuted()).toEqual(['parent-executed']);
  });

  test('editing the shared child hot-updates via the executed parent; the cold parent stays inert', async () => {
    await waitForBuildStable();
    await plantMarker();

    editFile('common-child.js', (code) => code.replace("'child-v1'", "'child-v2'"));
    await expect.poll(() => page.textContent('.child')).toBe('child-v2');

    // Hot, not a reload.
    expect(await readMarker()).toBe('alive');
    // The boundary re-ran the executed parent once more; `parent-cold` never executed —
    // its top-level tripwire would have pushed 'parent-cold'.
    expect(await readExecuted()).toEqual(['parent-executed', 'parent-executed']);
    await waitForBuildStable();
  });
});
