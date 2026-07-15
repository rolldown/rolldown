import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// `app` imports `foo` ONLY via a dynamic import() and accepts it as a dep with a
// callback. Editing `foo` must bubble across the dynamic edge to `app`'s accept-dep
// boundary and hot-update (the callback runs with the fresh module), not full-reload.

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = () =>
  page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

describe('hmr-dynamic-import-accept-dep', () => {
  test('renders the dynamically-imported value', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.foo')).toBe('foo-v1');
  });

  test('editing the dynamically-imported dep hot-updates via the importer accept-dep', async () => {
    await waitForBuildStable();
    await plantMarker();

    editFile('foo.js', (code) => code.replace("'foo-v1'", "'foo-v2'"));
    await expect.poll(() => page.textContent('.foo')).toBe('foo-v2');

    // No full reload happened: the boundary walk crossed the dynamic edge to `app`.
    expect(await readMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
