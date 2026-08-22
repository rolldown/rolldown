import { describe, expect, test } from 'vitest';
import {
  editFile,
  errorOverlayText,
  page,
  removeFile,
  waitForBuildStable,
} from '~utils';

// A module addressed as `<file>?query` gets its content from `<file>` through a plugin
// `load` hook (the html-proxy / `foo.vue?vue&type=...` pattern). Editing the file must
// re-fetch the `?query` variant along with it — exact-id invalidation alone would keep
// serving the variant's stale cached copy.

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = () =>
  page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

describe('hmr-query-variant', () => {
  test('renders the base module and its ?query variant', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.base')).toBe('hello');
    await expect.poll(() => page.textContent('.variant')).toBe('HELLO');
  });

  test('editing the file hot-updates the ?query variant too', async () => {
    await waitForBuildStable();
    await plantMarker();

    editFile('content.js', (code) => code.replace('hello', 'world'));
    await expect.poll(() => page.textContent('.base')).toBe('world');
    await expect.poll(() => page.textContent('.variant')).toBe('WORLD');
    expect(await readMarker()).toBe('alive');
    await waitForBuildStable();
  });

  test('deleting the file with an orphaned ?query variant is a quiet no-op', async () => {
    await waitForBuildStable();

    // Detach the importer first, so deleting the file leaves no broken import
    // behind — only the orphaned cached variant. Deleting it must NOT re-fetch
    // the variant: its `load` hook reads the deleted file and would turn a
    // no-op orphan deletion into a build error and overlay.
    editFile('main.js', () =>
      [
        '/* oxlint-disable */',
        "document.querySelector('.base').textContent = 'detached';",
        "document.querySelector('.variant').textContent = 'detached';",
        '',
        'import.meta.hot?.accept();',
        '',
      ].join('\n'));
    await expect.poll(() => page.textContent('.base')).toBe('detached');
    await waitForBuildStable();
    await plantMarker();

    removeFile('content.js');
    await waitForBuildStable();
    await expect.poll(errorOverlayText).toBe('');
    expect(await readMarker()).toBe('alive');
    expect(await page.textContent('.variant')).toBe('detached');
  });
});
