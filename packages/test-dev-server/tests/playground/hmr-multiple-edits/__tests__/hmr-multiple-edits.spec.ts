import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// Ports the retired node fixture `multiple-edits`: two modules edited in the same watch
// batch must arrive as ONE hot update — the importer's accept callback fires once with
// BOTH new values, never twice with a half-applied pair.

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = () =>
  page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

interface Update {
  foo: string;
  bar: string;
}
const readUpdates = () =>
  page.evaluate(() => (window as unknown as { __updates?: Update[] }).__updates ?? []);

describe('hmr-multiple-edits', () => {
  test('renders both values', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.foo')).toBe('foo-v1');
    await expect.poll(() => page.textContent('.bar')).toBe('bar-v1');
  });

  test('two files edited in one batch apply as a single hot update', async () => {
    await waitForBuildStable();
    await plantMarker();

    // Synchronous back-to-back writes: both land inside one watcher poll tick and one
    // debounce window, so they reach the dev engine as a single change batch.
    editFile('foo.js', (code) => code.replace("'foo-v1'", "'foo-v2'"));
    editFile('bar.js', (code) => code.replace("'bar-v1'", "'bar-v2'"));

    await expect.poll(() => page.textContent('.foo')).toBe('foo-v2');
    await expect.poll(() => page.textContent('.bar')).toBe('bar-v2');

    // Hot, not a reload.
    expect(await readMarker()).toBe('alive');
    // Exactly one accept callback, carrying both new values at once.
    expect(await readUpdates()).toEqual([{ foo: 'foo-v2', bar: 'bar-v2' }]);
    await waitForBuildStable();
  });
});
