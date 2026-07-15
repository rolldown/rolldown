import { describe, expect, test } from 'vitest';
import { editFile, page, waitForBuildStable } from '~utils';

// Ports the retired node fixture `transform-dependencies`: a plugin registers
// `config.json` via `this.addWatchFile` in its `transform` hook. Editing the watched
// file — which is not a module in the graph — must re-transform the watching module
// and hot-apply it through the self-accept, not full-reload.

/** Plant a marker on `window`; any full page reload wipes it. */
const plantMarker = () =>
  page.evaluate(() => ((window as unknown as { __marker?: string }).__marker = 'alive'));
const readMarker = () =>
  page.evaluate(() => (window as unknown as { __marker?: string }).__marker ?? null);

describe('hmr-watch-file-transform', () => {
  test('renders the injected config', async () => {
    await waitForBuildStable();
    await expect.poll(() => page.textContent('.config')).toBe('hello world v1');
  });

  test('editing the watched non-module file hot-updates the watching module', async () => {
    await waitForBuildStable();
    await plantMarker();

    editFile('config.json', (code) => code.replace('"version": 1', '"version": 2'));
    await expect.poll(() => page.textContent('.config')).toBe('hello world v2');
    expect(await readMarker()).toBe('alive');

    // The old fixture's `// @reload` step: force a full rebuild + fresh client session
    // in between. Editing a module nobody accepts walks to no boundary and full-reloads
    // onto a freshly rebuilt bundle.
    await waitForBuildStable();
    editFile('main.js', (code) => `${code}// reload step\n`);
    await expect.poll(() => readMarker()).toBe(null);
    await expect.poll(() => page.textContent('.config')).toBe('hello world v2');
    await waitForBuildStable();
    await plantMarker();

    // The watch registration survived the rebuild and the new session: the next edit
    // still lands, and still as a hot update.
    editFile('config.json', (code) => code.replace('"version": 2', '"version": 3'));
    await expect.poll(() => page.textContent('.config')).toBe('hello world v3');
    expect(await readMarker()).toBe('alive');
    await waitForBuildStable();
  });
});
